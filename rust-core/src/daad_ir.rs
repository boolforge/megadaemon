//! MegaDaemon v13.4 — Pure Rust DAAD DDB Intermediate Representation & Interpreter Fingerprinter.
//!
//! Decodes all structured sections of a verified DAAD database (DDB):
//! - Compressed text token table (`0x80`-terminated tokens)
//! - Messages & system messages (XOR `0xFF` decoding + token expansion)
//! - Vocabulary (7-byte records: 5 XOR-`0xFF` chars + word ID + word type)
//! - Locations & directional connection tables
//! - Objects (descriptions, initial locations, weight/container/wearable flags, noun/adjective bindings)
//! - Process tables (Verb/Noun entries + disassembled CondAct streams via [`CondActDisassembler`])
//!
//! Also provides official & community DAAD interpreter binary fingerprinting via
//! SHA-256 profile matching, qualified runtime filenames, and structural binary markers.

use crate::daad::{
    CondActDisassembler, DAADBytecodeParser, DDBAnalysis, DDBHeader, DisassembledCondAct,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Vocabulary Types
// ============================================================================

/// Canonical DAAD vocabulary word type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VocabWordType {
    Verb,
    Adverb,
    Noun,
    Adjective,
    Preposition,
    Conjunction,
    Pronoun,
    Unknown(u8),
}

impl VocabWordType {
    pub fn from_byte(code: u8) -> Self {
        match code {
            0 => Self::Verb,
            1 => Self::Adverb,
            2 => Self::Noun,
            3 => Self::Adjective,
            4 => Self::Preposition,
            5 => Self::Conjunction,
            6 => Self::Pronoun,
            other => Self::Unknown(other),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Verb => "verb",
            Self::Adverb => "adverb",
            Self::Noun => "noun",
            Self::Adjective => "adjective",
            Self::Preposition => "preposition",
            Self::Conjunction => "conjunction",
            Self::Pronoun => "pronoun",
            Self::Unknown(_) => "unknown",
        }
    }
}

/// One decoded 7-byte DAAD vocabulary record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VocabEntry {
    pub offset: usize,
    pub word: String,
    pub word_id: u8,
    pub word_type: VocabWordType,
    pub word_type_raw: u8,
}

// ============================================================================
// Text, Tokens, Locations, Objects, Processes
// ============================================================================

/// One decoded text compression token (byte values `0x80..=0xFF`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextToken {
    pub index: u8,
    pub offset: usize,
    pub text: String,
    pub raw_bytes: Vec<u8>,
}

/// One indexed DAAD text entry (message, system message, location, or object description).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedText {
    pub id: u8,
    pub offset: usize,
    pub text: String,
    pub raw_decoded: Vec<u8>,
}

/// One directional exit from a location.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationConnection {
    pub verb_id: u8,
    pub verb_word: Option<String>,
    pub destination: u8,
}

/// Complete decoded location entry (description + outgoing connections).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationEntry {
    pub id: u8,
    pub description_offset: Option<usize>,
    pub description: String,
    pub connections: Vec<LocationConnection>,
}

/// Complete decoded object entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectEntry {
    pub id: u8,
    pub description_offset: Option<usize>,
    pub description: String,
    pub initial_location: u8,
    pub weight: u8,
    pub is_container: bool,
    pub is_wearable: bool,
    pub raw_attributes: u8,
    pub extended_attributes: Option<u16>,
    pub noun_id: u8,
    pub adjective_id: u8,
    pub noun_word: Option<String>,
    pub adjective_word: Option<String>,
}

/// One Verb/Noun rule entry inside a DAAD process table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessEntry {
    pub offset: usize,
    pub verb_id: u8,
    pub verb_word: Option<String>,
    pub noun_id: u8,
    pub noun_word: Option<String>,
    pub condact_offset: usize,
    pub condacts: Vec<DisassembledCondAct>,
}

/// Complete decoded DAAD process table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessTable {
    pub id: u8,
    pub offset: usize,
    pub entries: Vec<ProcessEntry>,
}

/// Full intermediate representation of a parsed DAAD DDB database.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DDBDocument {
    pub header: DDBHeader,
    pub source_sha256: String,
    pub source_size: usize,
    pub tokens: Vec<TextToken>,
    pub vocabulary: Vec<VocabEntry>,
    pub messages: Vec<IndexedText>,
    pub system_messages: Vec<IndexedText>,
    pub locations: Vec<LocationEntry>,
    pub objects: Vec<ObjectEntry>,
    pub processes: Vec<ProcessTable>,
}

// ============================================================================
// Low-level Helpers
// ============================================================================

const DAAD_SPANISH_GLYPHS: &[char; 16] = &[
    'ª', '¡', '¿', '«', '»', 'á', 'é', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ç', 'Ç', 'ü', 'Ü',
];

fn read_word(data: &[u8], offset: usize, is_little_endian: bool) -> Option<u16> {
    if offset + 2 > data.len() {
        return None;
    }
    let bytes = [data[offset], data[offset + 1]];
    Some(if is_little_endian {
        u16::from_le_bytes(bytes)
    } else {
        u16::from_be_bytes(bytes)
    })
}

fn resolve_pointer(address: u16, base_address: u16, data_len: usize) -> Option<usize> {
    if address == 0 || address < base_address {
        return None;
    }
    let off = (address - base_address) as usize;
    if off < data_len {
        Some(off)
    } else {
        None
    }
}

fn push_daad_char(out: &mut String, byte: u8, is_spanish: bool) {
    match byte {
        0x0D => out.push('\n'),
        0x0E => out.push(' '),
        0x10..=0x1F if is_spanish => {
            out.push(DAAD_SPANISH_GLYPHS[(byte - 0x10) as usize]);
        }
        0x20..=0x7E => out.push(byte as char),
        _ => {}
    }
}

/// Decode a single DAAD compressed token table starting at `token_offset`.
///
/// Format: 1-byte block marker at `token_offset`, followed by up to 128 tokens
/// (indices `0..128`, corresponding to decoded text bytes `0x80..=0xFF`).
/// Each token's final byte has bit 7 (`0x80`) set; a `0x00` byte terminates early.
pub fn decode_tokens(data: &[u8], token_offset: usize, is_spanish: bool) -> Vec<TextToken> {
    if token_offset >= data.len() {
        return Vec::new();
    }
    let mut tokens = Vec::new();
    let mut pos = token_offset + 1; // skip 1-byte block marker

    for idx in 0u8..128u8 {
        if pos >= data.len() {
            break;
        }
        if data[pos] == 0x00 {
            break;
        }
        let start = pos;
        while pos < data.len() && (data[pos] & 0x80) == 0 && data[pos] != 0x00 {
            pos += 1;
        }
        if pos >= data.len() || data[pos] == 0x00 {
            break;
        }
        let end = pos + 1;
        let raw = &data[start..end];
        let mut decoded_bytes = Vec::with_capacity(raw.len());
        let mut text = String::with_capacity(raw.len());
        for (i, &b) in raw.iter().enumerate() {
            let ch = if i + 1 == raw.len() { b & 0x7F } else { b };
            decoded_bytes.push(ch);
            push_daad_char(&mut text, ch, is_spanish);
        }
        tokens.push(TextToken {
            index: idx,
            offset: start,
            text,
            raw_bytes: decoded_bytes,
        });
        pos = end;
    }

    tokens
}

/// Decode a single XOR-`0xFF` DAAD text string at `offset` (terminated by decoded `0x0A`)
/// and expand `0x80..=0xFF` token references using `tokens`.
pub fn decode_daad_text(
    data: &[u8],
    offset: usize,
    tokens: &[TextToken],
    is_spanish: bool,
) -> Option<(String, Vec<u8>)> {
    if offset >= data.len() {
        return None;
    }
    let mut pos = offset;
    let mut raw_decoded = Vec::new();
    let mut expanded = String::new();
    let max_scan = data.len().min(offset + 4096);

    while pos < max_scan {
        let decoded = data[pos] ^ 0xFF;
        pos += 1;
        if decoded == 0x0A {
            return Some((expanded, raw_decoded));
        }
        raw_decoded.push(decoded);
        if decoded >= 0x80 {
            let tok_idx = (decoded - 0x80) as usize;
            if let Some(tok) = tokens.get(tok_idx) {
                expanded.push_str(&tok.text);
            }
        } else {
            push_daad_char(&mut expanded, decoded, is_spanish);
        }
    }

    // If unterminated within scan window, return best-effort decoded text
    Some((expanded, raw_decoded))
}

/// Decode the DAAD vocabulary table starting at `vocab_offset`.
///
/// Each record is 7 bytes: 5 XOR-`0xFF` characters (masked to 7-bit ASCII),
/// `word_id` (`u8`), and `word_type` (`u8`). Terminated by a `0x00` byte.
pub fn decode_vocabulary(data: &[u8], vocab_offset: usize, is_spanish: bool) -> Vec<VocabEntry> {
    if vocab_offset >= data.len() {
        return Vec::new();
    }
    let mut entries = Vec::new();
    let mut pos = vocab_offset;
    let max_entries = 4096;

    while pos < data.len() && entries.len() < max_entries {
        if data[pos] == 0x00 {
            break;
        }
        if pos + 7 > data.len() {
            break;
        }
        let rec = &data[pos..pos + 7];
        let mut word = String::with_capacity(5);
        for &b in &rec[0..5] {
            let ch = (b ^ 0xFF) & 0x7F;
            push_daad_char(&mut word, ch, is_spanish);
        }
        let trimmed = word.trim_end().to_string();
        let word_id = rec[5];
        let word_type_raw = rec[6];
        entries.push(VocabEntry {
            offset: pos,
            word: trimmed,
            word_id,
            word_type: VocabWordType::from_byte(word_type_raw),
            word_type_raw,
        });
        pos += 7;
    }

    entries
}

fn read_pointer_table(
    data: &[u8],
    table_ptr: u16,
    count: usize,
    base_address: u16,
    is_le: bool,
) -> Vec<Option<usize>> {
    if count == 0 {
        return Vec::new();
    }
    let Some(table_off) = resolve_pointer(table_ptr, base_address, data.len()) else {
        return Vec::new();
    };
    let mut offsets = Vec::with_capacity(count);
    for i in 0..count {
        let Some(addr) = read_word(data, table_off + i * 2, is_le) else {
            break;
        };
        offsets.push(resolve_pointer(addr, base_address, data.len()));
    }
    offsets
}

// ============================================================================
// Main DDB IR Parser
// ============================================================================

/// Parse all structured DDB sections into a [`DDBDocument`].
///
/// Expects `data` to start at the DDB header (i.e. already sliced to `payload_offset..payload_offset+payload_size`).
pub fn parse_ddb_ir(data: &[u8], header: &DDBHeader) -> Result<DDBDocument, String> {
    if data.len() < header.header_size {
        return Err(format!(
            "DDB payload too short ({} bytes) for header size {}",
            data.len(),
            header.header_size
        ));
    }

    let is_le = header.endianness == "little";
    let is_spanish = header.language == "es";
    let base = header.base_address;
    let dialect = if header.major_version == 1 { "v1" } else { "v2" };

    // Compute SHA-256 of payload
    let mut hasher = Sha256::new();
    hasher.update(data);
    let source_sha256 = hex_lower(&hasher.finalize());

    // 1. Tokens table (pointer 0)
    let tokens = header
        .pointers
        .first()
        .copied()
        .and_then(|ptr| resolve_pointer(ptr, base, data.len()))
        .map(|off| decode_tokens(data, off, is_spanish))
        .unwrap_or_default();

    // 2. Vocabulary (pointer 7)
    let vocabulary = header
        .pointers
        .get(7)
        .copied()
        .and_then(|ptr| resolve_pointer(ptr, base, data.len()))
        .map(|off| decode_vocabulary(data, off, is_spanish))
        .unwrap_or_default();

    // Build quick lookup maps for vocabulary resolution
    let mut verb_map: HashMap<u8, String> = HashMap::new();
    let mut noun_map: HashMap<u8, String> = HashMap::new();
    let mut adj_map: HashMap<u8, String> = HashMap::new();
    for entry in &vocabulary {
        match entry.word_type {
            VocabWordType::Verb => {
                verb_map.entry(entry.word_id).or_insert_with(|| entry.word.clone());
            }
            VocabWordType::Noun => {
                noun_map.entry(entry.word_id).or_insert_with(|| entry.word.clone());
                // In DAAD, nouns with id < 20 can also act as convertible verbs
                if entry.word_id < 20 {
                    verb_map.entry(entry.word_id).or_insert_with(|| entry.word.clone());
                }
            }
            VocabWordType::Adjective => {
                adj_map.entry(entry.word_id).or_insert_with(|| entry.word.clone());
            }
            _ => {}
        }
    }

    // 3. Messages (pointer 4) & System Messages (pointer 5)
    let messages = parse_text_table(
        data,
        header.pointers.get(4).copied().unwrap_or(0),
        header.message_count as usize,
        base,
        is_le,
        &tokens,
        is_spanish,
    );
    let system_messages = parse_text_table(
        data,
        header.pointers.get(5).copied().unwrap_or(0),
        header.system_message_count as usize,
        base,
        is_le,
        &tokens,
        is_spanish,
    );

    // 4. Locations (pointer 3) & Connections (pointer 6)
    let loc_desc_ptrs = read_pointer_table(
        data,
        header.pointers.get(3).copied().unwrap_or(0),
        header.location_count as usize,
        base,
        is_le,
    );
    let conn_ptrs = read_pointer_table(
        data,
        header.pointers.get(6).copied().unwrap_or(0),
        header.location_count as usize,
        base,
        is_le,
    );

    let mut locations = Vec::with_capacity(header.location_count as usize);
    for id in 0..header.location_count {
        let idx = id as usize;
        let desc_off = loc_desc_ptrs.get(idx).copied().flatten();
        let description = desc_off
            .and_then(|off| decode_daad_text(data, off, &tokens, is_spanish))
            .map(|(t, _)| t)
            .unwrap_or_default();

        let mut connections = Vec::new();
        if let Some(conn_off) = conn_ptrs.get(idx).copied().flatten() {
            let mut pos = conn_off;
            while pos < data.len() && data[pos] != 0xFF {
                if pos + 2 > data.len() {
                    break;
                }
                let verb_id = data[pos];
                let destination = data[pos + 1];
                connections.push(LocationConnection {
                    verb_id,
                    verb_word: verb_map.get(&verb_id).cloned(),
                    destination,
                });
                pos += 2;
                if connections.len() >= 64 {
                    break;
                }
            }
        }

        locations.push(LocationEntry {
            id,
            description_offset: desc_off,
            description,
            connections,
        });
    }

    // 5. Objects: descriptions (ptr 2), initial locs (ptr 8), words (ptr 9), attrs (ptr 10), ext attrs (ptr 11)
    let obj_desc_ptrs = read_pointer_table(
        data,
        header.pointers.get(2).copied().unwrap_or(0),
        header.object_count as usize,
        base,
        is_le,
    );
    let obj_locs_off = header
        .pointers
        .get(8)
        .copied()
        .and_then(|p| resolve_pointer(p, base, data.len()));
    let obj_words_off = header
        .pointers
        .get(9)
        .copied()
        .and_then(|p| resolve_pointer(p, base, data.len()));
    let obj_attrs_off = header
        .pointers
        .get(10)
        .copied()
        .and_then(|p| resolve_pointer(p, base, data.len()));
    let obj_ext_attrs_off = if header.major_version >= 2 {
        header
            .pointers
            .get(11)
            .copied()
            .and_then(|p| resolve_pointer(p, base, data.len()))
    } else {
        None
    };

    let mut objects = Vec::with_capacity(header.object_count as usize);
    for id in 0..header.object_count {
        let idx = id as usize;
        let desc_off = obj_desc_ptrs.get(idx).copied().flatten();
        let description = desc_off
            .and_then(|off| decode_daad_text(data, off, &tokens, is_spanish))
            .map(|(t, _)| t)
            .unwrap_or_default();

        let initial_location = obj_locs_off
            .and_then(|base_off| data.get(base_off + idx).copied())
            .unwrap_or(252);

        let raw_attributes = obj_attrs_off
            .and_then(|base_off| data.get(base_off + idx).copied())
            .unwrap_or(0);

        let extended_attributes = obj_ext_attrs_off
            .and_then(|base_off| read_word(data, base_off + idx * 2, is_le));

        let (noun_id, adjective_id) = obj_words_off
            .and_then(|base_off| {
                if base_off + idx * 2 + 1 < data.len() {
                    Some((data[base_off + idx * 2], data[base_off + idx * 2 + 1]))
                } else {
                    None
                }
            })
            .unwrap_or((0xFF, 0xFF));

        let noun_word = if noun_id == 0xFF {
            None
        } else {
            noun_map.get(&noun_id).cloned()
        };
        let adjective_word = if adjective_id == 0xFF {
            None
        } else {
            adj_map.get(&adjective_id).cloned()
        };

        objects.push(ObjectEntry {
            id,
            description_offset: desc_off,
            description,
            initial_location,
            weight: raw_attributes & 0x3F,
            is_container: (raw_attributes & 0x40) != 0,
            is_wearable: (raw_attributes & 0x80) != 0,
            raw_attributes,
            extended_attributes,
            noun_id,
            adjective_id,
            noun_word,
            adjective_word,
        });
    }

    // 6. Processes (pointer 1)
    let proc_ptrs = read_pointer_table(
        data,
        header.pointers.get(1).copied().unwrap_or(0),
        header.process_count as usize,
        base,
        is_le,
    );
    let mut processes = Vec::with_capacity(proc_ptrs.len());
    for (proc_idx, proc_off_opt) in proc_ptrs.into_iter().enumerate() {
        let Some(proc_off) = proc_off_opt else {
            continue;
        };
        let mut entries = Vec::new();
        let mut pos = proc_off;
        while pos < data.len() && entries.len() < 512 {
            if data[pos] == 0x00 {
                break;
            }
            if pos + 4 > data.len() {
                break;
            }
            let verb_id = data[pos];
            let noun_id = data[pos + 1];
            let condact_addr = read_word(data, pos + 2, is_le).unwrap_or(0);
            let condact_offset = resolve_pointer(condact_addr, base, data.len()).unwrap_or(0);
            let condacts = if condact_offset > 0 && condact_offset < data.len() {
                let (disasm, _) = CondActDisassembler::disassemble_stream(data, condact_offset, dialect);
                disasm
            } else {
                Vec::new()
            };

            let verb_word = if verb_id == 0xFF {
                Some("_".to_string())
            } else {
                verb_map.get(&verb_id).cloned()
            };
            let noun_word = if noun_id == 0xFF {
                Some("_".to_string())
            } else {
                noun_map.get(&noun_id).cloned()
            };

            entries.push(ProcessEntry {
                offset: pos,
                verb_id,
                verb_word,
                noun_id,
                noun_word,
                condact_offset,
                condacts,
            });
            pos += 4;
        }

        processes.push(ProcessTable {
            id: proc_idx as u8,
            offset: proc_off,
            entries,
        });
    }

    Ok(DDBDocument {
        header: header.clone(),
        source_sha256,
        source_size: data.len(),
        tokens,
        vocabulary,
        messages,
        system_messages,
        locations,
        objects,
        processes,
    })
}

/// Convenience helper: validate raw bytes with [`DAADBytecodeParser`] and parse into [`DDBDocument`].
pub fn parse_ddb_ir_auto(data: &[u8], filename: &str) -> Result<DDBDocument, String> {
    let analysis = DAADBytecodeParser::parse_ddb(data, filename);
    let (payload, header) = if analysis.is_daad {
        let hdr = analysis
            .header
            .ok_or_else(|| "Verified DDB missing parsed header".to_string())?;
        let end = (analysis.payload_offset + analysis.payload_size).min(data.len());
        (&data[analysis.payload_offset..end], hdr)
    } else if let Some((off, size, emb)) = DAADBytecodeParser::find_embedded_ddb(data) {
        let hdr = emb
            .header
            .ok_or_else(|| "Embedded DDB missing parsed header".to_string())?;
        let end = (off + size).min(data.len());
        (&data[off..end], hdr)
    } else {
        return Err(format!("Not a valid DAAD DDB: {}", analysis.reason));
    };

    parse_ddb_ir(payload, &header)
}

fn parse_text_table(
    data: &[u8],
    table_ptr: u16,
    count: usize,
    base_address: u16,
    is_le: bool,
    tokens: &[TextToken],
    is_spanish: bool,
) -> Vec<IndexedText> {
    let ptrs = read_pointer_table(data, table_ptr, count, base_address, is_le);
    let mut out = Vec::with_capacity(ptrs.len());
    for (idx, off_opt) in ptrs.into_iter().enumerate() {
        let Some(off) = off_opt else {
            continue;
        };
        if let Some((text, raw_decoded)) = decode_daad_text(data, off, tokens, is_spanish) {
            out.push(IndexedText {
                id: idx as u8,
                offset: off,
                text,
                raw_decoded,
            });
        }
    }
    out
}

// ============================================================================
// Interpreter Fingerprinting & Profile Registry
// ============================================================================

/// Known DAAD interpreter binary profile (official & community runtimes).
#[derive(Clone, Copy, Debug)]
pub struct InterpreterProfile {
    pub profile_id: &'static str,
    pub platform: &'static str,
    pub filenames: &'static [&'static str],
    pub sha256: Option<&'static str>,
    pub interpreter_version: Option<&'static str>,
    pub language: Option<&'static str>,
}

/// Official & community DAAD interpreter profiles (matching `interpreter_profiles.py`).
pub const INTERPRETER_PROFILES: &[InterpreterProfile] = &[
    // ZX Spectrum
    InterpreterProfile {
        profile_id: "daad-zx-ds48ie-official",
        platform: "zx",
        filenames: &["ds48ie.p3f", "ds48ie.bin", "ds48ie"],
        sha256: Some("d599b14576cb46f72b34b106f6ad1079cdf0d346ed9ac44e89db7dbd0c0eb3d6"),
        interpreter_version: Some("official-zx-en"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-zx-ds48is-official",
        platform: "zx",
        filenames: &["ds48is.p3f", "ds48is.bin", "ds48is"],
        sha256: Some("e1e0f8ab8e2907eebeb24ada1bf2bb2d27b0622972239827eeb70f56ff5e0566"),
        interpreter_version: Some("official-zx-es"),
        language: Some("es"),
    },
    // Amstrad CPC
    InterpreterProfile {
        profile_id: "daad-cpc-dcpcie-official",
        platform: "cpc",
        filenames: &["dcpcie.z80", "dcpcie.bin", "dcpcie"],
        sha256: Some("7447b56073a6e65242214c505496742346a35de0e81777ed91ea44a2f115ee41"),
        interpreter_version: Some("official-cpc-en"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-cpc-dcpcis-official",
        platform: "cpc",
        filenames: &["dcpcis.z80", "dcpcis.bin", "dcpcis"],
        sha256: Some("8d312860b4ce8596b9f6152c1fbc0aa5f3525189daa564dc413bf3173a19659c"),
        interpreter_version: Some("official-cpc-es"),
        language: Some("es"),
    },
    // Commodore 64
    InterpreterProfile {
        profile_id: "daad-c64-edi64-official",
        platform: "c64",
        filenames: &["edi64.prg", "edi64"],
        sha256: Some("c462192db2a552f079aa7d521f3ae23a3d5c066120b42108a905085e923932c5"),
        interpreter_version: Some("official-c64-en"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-c64-sdi64-official",
        platform: "c64",
        filenames: &["sdi64.prg", "sdi64"],
        sha256: Some("767aa97d411ea025d20e25282ae0ac90360b4fb99af01392c38d2721aa7033ec"),
        interpreter_version: Some("official-c64-es"),
        language: Some("es"),
    },
    // Commodore Plus/4
    InterpreterProfile {
        profile_id: "daad-plus4-ediplus4-official",
        platform: "plus4",
        filenames: &["ediplus4.prg"],
        sha256: Some("9df2d6b35d5e138c80e3a38674afd565fe9564f27205d79d24d654d407949523"),
        interpreter_version: Some("official-plus4-en"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-plus4-ediplus4-historical",
        platform: "plus4",
        filenames: &["ediplus4"],
        sha256: None,
        interpreter_version: None,
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-plus4-sdiplus4-official",
        platform: "plus4",
        filenames: &["sdiplus4.prg", "sdiplus4"],
        sha256: Some("f29050dfe9331634b985a8c5b8cb4ee3b164a4340e355c044284168b8610258c"),
        interpreter_version: Some("official-plus4-es"),
        language: Some("es"),
    },
    // MSX
    InterpreterProfile {
        profile_id: "daad-msx-msxedi-official",
        platform: "msx",
        filenames: &["msxedi.z80"],
        sha256: Some("1df91cff49dc2dcb42f2e4321644b6e088a0ac63159444bb937a9bff5848a4ca"),
        interpreter_version: Some("official-msx-en"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-msx-msxsdi-official",
        platform: "msx",
        filenames: &["msxsdi.z80"],
        sha256: Some("fcae12bd09bc949e1e6cf75b3d1ef87e66c322efe2de005e771fa3b5dd8ddbf8"),
        interpreter_version: Some("official-msx-es"),
        language: Some("es"),
    },
    // Amstrad PCW
    InterpreterProfile {
        profile_id: "daad-pcw-pcwedi-official",
        platform: "pcw",
        filenames: &["pcwedi.bin"],
        sha256: Some("358eb0d236db0a2b43fdc66f843eb8e2eed097f77db24e195c14bae5fbb309a2"),
        interpreter_version: Some("official-pcw-en"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-pcw-pcwsdi-official",
        platform: "pcw",
        filenames: &["pcwsdi.bin"],
        sha256: Some("e52e2a8cc9e720805c7c6c7f8fe9fd706933c73696e695d7c1132063d4e52a7d"),
        interpreter_version: Some("official-pcw-es"),
        language: Some("es"),
    },
    // Atari ST
    InterpreterProfile {
        profile_id: "daad-atarist-edi1-official",
        platform: "atarist",
        filenames: &["edi1.prg"],
        sha256: Some("ecc25e01611b1b3f2546c63b67220011f1793809cb177543030d5b626520c7fd"),
        interpreter_version: Some("official-st-edi1"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-atarist-edi2-official",
        platform: "atarist",
        filenames: &["edi2.prg"],
        sha256: Some("6e1c18327066ead24985d6b9033353f16ca7e70e39863af5320d933194b38a72"),
        interpreter_version: Some("official-st-edi2"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-atarist-sdi1-official",
        platform: "atarist",
        filenames: &["sdi1.prg"],
        sha256: Some("35a0c10701e59739e1bf7e701e1e1aa7f5bb5112f79445e8038be0502360a9f2"),
        interpreter_version: Some("official-st-sdi1"),
        language: Some("es"),
    },
    InterpreterProfile {
        profile_id: "daad-atarist-sdi2-official",
        platform: "atarist",
        filenames: &["sdi2.prg"],
        sha256: Some("04ac433d70777dc15434ec5d7c7002f28b30bc8f1f3ed5c0eef01ea6c245a9b1"),
        interpreter_version: Some("official-st-sdi2"),
        language: Some("es"),
    },
    // Amiga
    InterpreterProfile {
        profile_id: "daad-amiga-edi1-official",
        platform: "amiga",
        filenames: &["edi1", "edi1.prg"],
        sha256: Some("5f3bd8275d500d44fc41d7ae2a77e86593bd7075b6ad9fffde639265c0990952"),
        interpreter_version: Some("official-amiga-edi1"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-amiga-edi2-official",
        platform: "amiga",
        filenames: &["edi2", "edi2.prg"],
        sha256: Some("6926ca43824c00098dcb6267a639aede2ae0640b2640be03fa9a1be965f37664"),
        interpreter_version: Some("official-amiga-edi2"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-amiga-sdi1-official",
        platform: "amiga",
        filenames: &["sdi1", "sdi1.prg"],
        sha256: Some("105c27fc80bcb9183409d9774792a8486ec34207d9a29e21efc53e475558c7f7"),
        interpreter_version: Some("official-amiga-sdi1"),
        language: Some("es"),
    },
    InterpreterProfile {
        profile_id: "daad-amiga-sdi2-official",
        platform: "amiga",
        filenames: &["sdi2", "sdi2.prg"],
        sha256: Some("e083dcd70726c97204404fa48eea9db49d839a6af4218408c4d40520dd8ec338"),
        interpreter_version: Some("official-amiga-sdi2"),
        language: Some("es"),
    },
    // DOS (English)
    InterpreterProfile {
        profile_id: "daad-dos-inte1-official",
        platform: "dos",
        filenames: &["inte1.exe"],
        sha256: Some("1070941c6fa8e73a3b5357be72db97d1f1a6b48ab01fbe895470586ef3a47617"),
        interpreter_version: Some("official-dos-inte1"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-inte2-official",
        platform: "dos",
        filenames: &["inte2.exe"],
        sha256: Some("4cb3a608f331c20402ab1ad3c13968e86f3c909dc1ebad31d7a3e3b50312837f"),
        interpreter_version: Some("official-dos-inte2"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-inte3-official",
        platform: "dos",
        filenames: &["inte3.exe"],
        sha256: Some("ea2494b657626db967f36d608b8cabe9ab82832f5d38266c8f1b6e40b9eb7aa3"),
        interpreter_version: Some("official-dos-inte3"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intecga1-official",
        platform: "dos",
        filenames: &["intecga1.exe"],
        sha256: Some("665ed57b5ad4b4168a70cc11634159081fc4f1f7ab57b49c75245755efb261c7"),
        interpreter_version: Some("official-dos-intecga1"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intecga2-official",
        platform: "dos",
        filenames: &["intecga2.exe"],
        sha256: Some("ddc34e84977308c3f592d304473d8398c84b4d70666f0c35aed74e3b88b76d6d"),
        interpreter_version: Some("official-dos-intecga2"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intecga3-official",
        platform: "dos",
        filenames: &["intecga3.exe"],
        sha256: Some("bdaaafe615ec073d81757e81504b3ffbed337a9bd4729d0aff203a4cbabadb4a"),
        interpreter_version: Some("official-dos-intecga3"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-inted-official",
        platform: "dos",
        filenames: &["inted.exe"],
        sha256: Some("d683e9c9a023e9996d811aa1a803bcc98d0fb3c91055f68ad3ceca4a59d1f9c9"),
        interpreter_version: Some("official-dos-inted"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intedm-official",
        platform: "dos",
        filenames: &["intedm.exe"],
        sha256: Some("3e0d786ce8abdd3225af8f290a19fb3b9ba2fdc32106b401b6c7028e3e5065e1"),
        interpreter_version: Some("official-dos-intedm"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intem1-official",
        platform: "dos",
        filenames: &["intem1.exe"],
        sha256: Some("6b50665f29c1024cbad8fe7ee5be42e0eb3ad49e1db8071119bf0e9e1ed57594"),
        interpreter_version: Some("official-dos-intem1"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intem2-official",
        platform: "dos",
        filenames: &["intem2.exe"],
        sha256: Some("2ca22c899432c1ab0d96418961f91638ffd3438a8cf4e8f69b8f9419a0dec765"),
        interpreter_version: Some("official-dos-intem2"),
        language: Some("en"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intem3-official",
        platform: "dos",
        filenames: &["intem3.exe"],
        sha256: Some("a430e15fe634157eae3fc45324787a3e6b79fa15814cb98e709c71407d55c5fc"),
        interpreter_version: Some("official-dos-intem3"),
        language: Some("en"),
    },
    // DOS (Spanish)
    InterpreterProfile {
        profile_id: "daad-dos-ints1-official",
        platform: "dos",
        filenames: &["ints1.exe"],
        sha256: Some("af5a3b732f53ee75042715f618e8a3daec77833491b8582d428ad2ba203129b5"),
        interpreter_version: Some("official-dos-ints1"),
        language: Some("es"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-ints2-official",
        platform: "dos",
        filenames: &["ints2.exe"],
        sha256: Some("39d7d99a40d77f466e648ded927b3c02bdef779411f7151ca51219be360a6455"),
        interpreter_version: Some("official-dos-ints2"),
        language: Some("es"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-ints3-official",
        platform: "dos",
        filenames: &["ints3.exe"],
        sha256: Some("fef8c5eabf256094da57dae6787884243ca3dec98dbe7a3c7438c93509903de3"),
        interpreter_version: Some("official-dos-ints3"),
        language: Some("es"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intscga1-official",
        platform: "dos",
        filenames: &["intscga1.exe"],
        sha256: Some("bd36734b7909453a3784dfd9f819ba505a82df21f973a9992e58bec58f1e6191"),
        interpreter_version: Some("official-dos-intscga1"),
        language: Some("es"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intscga2-official",
        platform: "dos",
        filenames: &["intscga2.exe"],
        sha256: Some("c478647bba2a3330c89c4233d3b51ba36d6357d7d7d30c7fe270287eab9034e6"),
        interpreter_version: Some("official-dos-intscga2"),
        language: Some("es"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intscga3-official",
        platform: "dos",
        filenames: &["intscga3.exe"],
        sha256: Some("6a8f60d5dcbf5e650934eb452ddfd215325bc5a02fdaf3b9ddf4937afb765cb9"),
        interpreter_version: Some("official-dos-intscga3"),
        language: Some("es"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intsd-official",
        platform: "dos",
        filenames: &["intsd.exe"],
        sha256: Some("797fcca14224a71cd03a02f476d12513f1d360921b24db2502544841b90a0567"),
        interpreter_version: Some("official-dos-intsd"),
        language: Some("es"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intsdm-official",
        platform: "dos",
        filenames: &["intsdm.exe"],
        sha256: Some("3a8187daf5ab46d3b6deaba955128477e13b5d62713e06f607fa040273514937"),
        interpreter_version: Some("official-dos-intsdm"),
        language: Some("es"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intsm1-official",
        platform: "dos",
        filenames: &["intsm1.exe"],
        sha256: Some("b17f78e22dd71961a21eb94e7afbd85cc1ec104e3860339f40d1e1877db5be51"),
        interpreter_version: Some("official-dos-intsm1"),
        language: Some("es"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intsm2-official",
        platform: "dos",
        filenames: &["intsm2.exe"],
        sha256: Some("27531111a82e9f5f62c6eae7efcc03175542a5ffe6f8d85677e88b3d4db16dab"),
        interpreter_version: Some("official-dos-intsm2"),
        language: Some("es"),
    },
    InterpreterProfile {
        profile_id: "daad-dos-intsm3-official",
        platform: "dos",
        filenames: &["intsm3.exe"],
        sha256: Some("54baa0b227cd2c1849c90d6727de130c58cda3a8c0d8c2f8adf50585d3246f01"),
        interpreter_version: Some("official-dos-intsm3"),
        language: Some("es"),
    },
    // Community modern DAAD interpreters
    InterpreterProfile {
        profile_id: "daad-dos-pcdaad-community",
        platform: "dos",
        filenames: &["pcdaad.exe", "pcdaad.com", "pcdaad"],
        sha256: None,
        interpreter_version: Some("pcdaad"),
        language: None,
    },
    InterpreterProfile {
        profile_id: "daad-msx2-msx2daad-community",
        platform: "msx2",
        filenames: &["msx2daad.com", "msx2daad.bin", "msx2daad"],
        sha256: None,
        interpreter_version: Some("msx2daad"),
        language: None,
    },
];

/// Identified DAAD interpreter match.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpreterMatch {
    pub profile_id: String,
    pub platform: String,
    pub confidence: String, // "verified" | "strong" | "heuristic"
    pub match_kind: String, // "sha256" | "filename" | "marker"
    pub filename: String,
    pub sha256: String,
    pub interpreter_version: Option<String>,
    pub language: Option<String>,
}

/// Complete forensic fingerprint report for an arbitrary binary blob.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FingerprintReport {
    pub filename: String,
    pub size: usize,
    pub sha256: String,
    pub is_ddb: bool,
    pub embedded_offset: Option<usize>,
    pub ddb_analysis: Option<DDBAnalysis>,
    pub interpreter_match: Option<InterpreterMatch>,
    pub structural_markers: Vec<String>,
}

/// Fingerprint a binary blob for DAAD DDB structure and official/community interpreter identity.
pub fn fingerprint_binary(data: &[u8], filename: &str) -> FingerprintReport {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hex_lower(&hasher.finalize());

    let base_name = Path::new(filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(filename)
        .to_lowercase();

    // 1. Structural DDB analysis (direct or embedded)
    let direct = DAADBytecodeParser::parse_ddb(data, filename);
    let (is_ddb, embedded_offset, ddb_analysis) = if direct.is_daad {
        (true, None, Some(direct))
    } else if direct.reason != "raw_archive_header" {
        if let Some((off, _size, emb)) = DAADBytecodeParser::find_embedded_ddb(data) {
            (true, Some(off), Some(emb))
        } else {
            (false, None, None)
        }
    } else {
        (false, None, None)
    };

    // 2. Scan structural markers inside the binary
    let markers = scan_structural_markers(data);

    // 3. Interpreter matching: exact SHA-256 -> qualified filename -> binary marker heuristics
    let interpreter_match = identify_interpreter_bytes(data, &base_name, &digest, &markers);

    FingerprintReport {
        filename: filename.to_string(),
        size: data.len(),
        sha256: digest,
        is_ddb,
        embedded_offset,
        ddb_analysis,
        interpreter_match,
        structural_markers: markers,
    }
}

fn identify_interpreter_bytes(
    data: &[u8],
    lower_name: &str,
    digest: &str,
    markers: &[String],
) -> Option<InterpreterMatch> {
    // 1. Exact SHA-256 match against official profile
    if let Some(p) = INTERPRETER_PROFILES
        .iter()
        .find(|p| p.sha256 == Some(digest))
    {
        return Some(InterpreterMatch {
            profile_id: p.profile_id.to_string(),
            platform: p.platform.to_string(),
            confidence: "verified".to_string(),
            match_kind: "sha256".to_string(),
            filename: lower_name.to_string(),
            sha256: digest.to_string(),
            interpreter_version: p.interpreter_version.map(|s| s.to_string()),
            language: p.language.map(|s| s.to_string()),
        });
    }

    // 2. Qualified runtime filename match
    if let Some(p) = INTERPRETER_PROFILES
        .iter()
        .find(|p| p.filenames.contains(&lower_name))
    {
        return Some(InterpreterMatch {
            profile_id: p.profile_id.to_string(),
            platform: p.platform.to_string(),
            confidence: "strong".to_string(),
            match_kind: "filename".to_string(),
            filename: lower_name.to_string(),
            sha256: digest.to_string(),
            interpreter_version: p.interpreter_version.map(|s| s.to_string()),
            language: p.language.map(|s| s.to_string()),
        });
    }

    // 3. Heuristic structural marker matching for renamed community/official runtimes
    let has = |m: &str| markers.iter().any(|x| x == m);
    if has("PCDAAD") {
        return Some(InterpreterMatch {
            profile_id: "daad-dos-pcdaad-community".into(),
            platform: "dos".into(),
            confidence: "heuristic".into(),
            match_kind: "marker".into(),
            filename: lower_name.to_string(),
            sha256: digest.to_string(),
            interpreter_version: Some("pcdaad".into()),
            language: None,
        });
    }
    if has("MSX2DAAD") {
        return Some(InterpreterMatch {
            profile_id: "daad-msx2-msx2daad-community".into(),
            platform: "msx2".into(),
            confidence: "heuristic".into(),
            match_kind: "marker".into(),
            filename: lower_name.to_string(),
            sha256: digest.to_string(),
            interpreter_version: Some("msx2daad".into()),
            language: None,
        });
    }
    if has("Aventuras AD") || has("Infinite Imaginations") {
        let platform = if data.starts_with(b"MZ") {
            "dos"
        } else if data.starts_with(b"PLUS3DOS\x1a") {
            "zx"
        } else if data.starts_with(b"\x00\x00\x03\xf3") {
            "amiga"
        } else if data.starts_with(b"\x60\x1a") {
            "atarist"
        } else {
            "unknown"
        };
        return Some(InterpreterMatch {
            profile_id: format!("daad-{}-marker-runtime", platform),
            platform: platform.into(),
            confidence: "heuristic".into(),
            match_kind: "marker".into(),
            filename: lower_name.to_string(),
            sha256: digest.to_string(),
            interpreter_version: None,
            language: if has("Aventuras AD") {
                Some("es".into())
            } else {
                None
            },
        });
    }

    None
}

fn scan_structural_markers(data: &[u8]) -> Vec<String> {
    let scan_slice = &data[..data.len().min(262_144)];
    let needles: &[(&str, &[u8])] = &[
        ("PCDAAD", b"PCDAAD"),
        ("MSX2DAAD", b"MSX2DAAD"),
        ("Aventuras AD", b"Aventuras AD"),
        ("Infinite Imaginations", b"Infinite Imaginations"),
        ("DS48IS", b"DS48IS"),
        ("DS48IE", b"DS48IE"),
        ("DCPCIS", b"DCPCIS"),
        ("DCPCIE", b"DCPCIE"),
        ("SDI64", b"SDI64"),
        ("EDI64", b"EDI64"),
        ("PLUS3DOS", b"PLUS3DOS\x1a"),
        ("DOS-MZ", b"MZ"),
        ("Amiga-Hunk", b"\x00\x00\x03\xf3"),
        ("AtariST-PRG", b"\x60\x1a"),
    ];

    let mut found = Vec::new();
    for &(label, needle) in needles {
        let matched = if label == "DOS-MZ" || label == "Amiga-Hunk" || label == "AtariST-PRG" || label == "PLUS3DOS" {
            scan_slice.starts_with(needle)
        } else {
            contains_case_insensitive(scan_slice, needle)
        };
        if matched {
            found.push(label.to_string());
        }
    }
    found
}

fn contains_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| {
        w.iter()
            .zip(needle.iter())
            .all(|(&a, &b)| a.to_ascii_lowercase() == b.to_ascii_lowercase())
    })
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
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_daad_word(word: &str, id: u8, wtype: u8) -> [u8; 7] {
        let mut out = [0u8; 7];
        let bytes = word.as_bytes();
        for i in 0..5 {
            let ch = if i < bytes.len() { bytes[i] } else { b' ' };
            out[i] = ch ^ 0xFF;
        }
        out[5] = id;
        out[6] = wtype;
        out
    }

    fn encode_daad_string(raw: &[u8]) -> Vec<u8> {
        let mut out: Vec<u8> = raw.iter().map(|&b| b ^ 0xFF).collect();
        out.push(0x0A ^ 0xFF);
        out
    }

    #[test]
    fn test_vocabulary_decoding() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&encode_daad_word("NORTE", 1, 0)); // Verb
        buf.extend_from_slice(&encode_daad_word("LLAVE", 20, 2)); // Noun
        buf.extend_from_slice(&encode_daad_word("ROJA ", 5, 3)); // Adjective
        buf.push(0x00); // terminator

        let vocab = decode_vocabulary(&buf, 0, true);
        assert_eq!(vocab.len(), 3);
        assert_eq!(vocab[0].word, "NORTE");
        assert_eq!(vocab[0].word_id, 1);
        assert_eq!(vocab[0].word_type, VocabWordType::Verb);
        assert_eq!(vocab[1].word, "LLAVE");
        assert_eq!(vocab[1].word_id, 20);
        assert_eq!(vocab[1].word_type, VocabWordType::Noun);
        assert_eq!(vocab[2].word, "ROJA");
        assert_eq!(vocab[2].word_id, 5);
        assert_eq!(vocab[2].word_type, VocabWordType::Adjective);
    }

    #[test]
    fn test_token_and_xor_message_expansion() {
        // Token table: 1-byte marker (0x00), token 0 = " the ", token 1 = "key"
        let mut tok_buf = vec![0x00];
        tok_buf.extend_from_slice(b" the");
        tok_buf.push(b' ' | 0x80);
        tok_buf.extend_from_slice(b"ke");
        tok_buf.push(b'y' | 0x80);
        tok_buf.push(0x00);

        let tokens = decode_tokens(&tok_buf, 0, false);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].text, " the ");
        assert_eq!(tokens[1].text, "key");

        // Message: "Take" + token 0 + token 1 + "."
        let msg_raw = [b'T', b'a', b'k', b'e', 0x80, 0x81, b'.'];
        let encoded = encode_daad_string(&msg_raw);
        let (decoded, _) = decode_daad_text(&encoded, 0, &tokens, false).unwrap();
        assert_eq!(decoded, "Take the key.");
    }

    #[test]
    fn test_synthetic_ddb_ir_extraction() {
        // Build a complete synthetic DRC v2 ZX Spectrum DDB with all sections populated
        let base: u16 = 0x8400;
        let mut data = vec![0u8; 640];

        // Header
        data[0] = 2; // major version 2
        data[1] = 0x10; // ZX (0x10) + English (0x00)
        data[2] = 95; // submachine 0x5F
        data[3] = 1; // 1 object
        data[4] = 2; // 2 locations
        data[5] = 1; // 1 message
        data[6] = 1; // 1 sys message
        data[7] = 1; // 1 process

        let set_ptr = |buf: &mut [u8], idx: usize, off: usize| {
            let addr = base + off as u16;
            buf[8 + idx * 2] = (addr >> 8) as u8;
            buf[8 + idx * 2 + 1] = (addr & 0xFF) as u8;
        };
        let write_be16 = |buf: &mut [u8], off: usize, val: u16| {
            buf[off] = (val >> 8) as u8;
            buf[off + 1] = (val & 0xFF) as u8;
        };

        // 0: Tokens at 0x40 (64)
        let off_tokens = 64;
        set_ptr(&mut data, 0, off_tokens);
        data[off_tokens] = 0x00;
        data[off_tokens + 1] = b'r';
        data[off_tokens + 2] = b'o';
        data[off_tokens + 3] = b'o';
        data[off_tokens + 4] = b'm' | 0x80;
        data[off_tokens + 5] = 0x00;

        // 7: Vocabulary at 0x50 (80)
        let off_vocab = 80;
        set_ptr(&mut data, 7, off_vocab);
        data[off_vocab..off_vocab + 7].copy_from_slice(&encode_daad_word("NORTH", 1, 0));
        data[off_vocab + 7..off_vocab + 14].copy_from_slice(&encode_daad_word("LAMP ", 20, 2));
        data[off_vocab + 14..off_vocab + 21].copy_from_slice(&encode_daad_word("BRASS", 7, 3));
        data[off_vocab + 21] = 0x00;

        // Strings area starting at 0x80 (128)
        let off_obj0_str = 128;
        let obj0_enc = encode_daad_string(b"a brass lamp");
        data[off_obj0_str..off_obj0_str + obj0_enc.len()].copy_from_slice(&obj0_enc);

        let off_loc0_str = 150;
        let loc0_enc = encode_daad_string(&[b'A', b' ', b'd', b'a', b'r', b'k', b' ', 0x80, b'.']);
        data[off_loc0_str..off_loc0_str + loc0_enc.len()].copy_from_slice(&loc0_enc);

        let off_loc1_str = 170;
        let loc1_enc = encode_daad_string(b"Hallway.");
        data[off_loc1_str..off_loc1_str + loc1_enc.len()].copy_from_slice(&loc1_enc);

        let off_msg0_str = 190;
        let msg0_enc = encode_daad_string(b"Hello DAAD!");
        data[off_msg0_str..off_msg0_str + msg0_enc.len()].copy_from_slice(&msg0_enc);

        let off_sys0_str = 210;
        let sys0_enc = encode_daad_string(b"OK.");
        data[off_sys0_str..off_sys0_str + sys0_enc.len()].copy_from_slice(&sys0_enc);

        // Pointer tables
        // 2: object_names_table at 240
        let off_obj_names = 240;
        set_ptr(&mut data, 2, off_obj_names);
        write_be16(&mut data, off_obj_names, base + off_obj0_str as u16);

        // 3: location_descriptions_table at 250
        let off_loc_descs = 250;
        set_ptr(&mut data, 3, off_loc_descs);
        write_be16(&mut data, off_loc_descs, base + off_loc0_str as u16);
        write_be16(&mut data, off_loc_descs + 2, base + off_loc1_str as u16);

        // 4: messages_table at 260
        let off_msgs = 260;
        set_ptr(&mut data, 4, off_msgs);
        write_be16(&mut data, off_msgs, base + off_msg0_str as u16);

        // 5: system_messages_table at 270
        let off_sys = 270;
        set_ptr(&mut data, 5, off_sys);
        write_be16(&mut data, off_sys, base + off_sys0_str as u16);

        // 6: connections_table at 280 -> loc0 conn at 290, loc1 conn at 296
        let off_conns = 280;
        set_ptr(&mut data, 6, off_conns);
        write_be16(&mut data, off_conns, base + 290);
        write_be16(&mut data, off_conns + 2, base + 296);
        data[290] = 1; // NORTH
        data[291] = 1; // -> loc 1
        data[292] = 0xFF;
        data[296] = 0xFF;

        // 8: object_locations_table at 300
        let off_obj_locs = 300;
        set_ptr(&mut data, 8, off_obj_locs);
        data[off_obj_locs] = 0; // loc 0

        // 9: object_words_table at 310
        let off_obj_words = 310;
        set_ptr(&mut data, 9, off_obj_words);
        data[off_obj_words] = 20; // LAMP
        data[off_obj_words + 1] = 7; // BRASS

        // 10: object_attributes_table at 320
        let off_obj_attrs = 320;
        set_ptr(&mut data, 10, off_obj_attrs);
        data[off_obj_attrs] = 0x80 | 0x40 | 5; // wearable + container + weight 5

        // 11: extended_object_attributes_table at 330
        let off_obj_ext = 330;
        set_ptr(&mut data, 11, off_obj_ext);
        write_be16(&mut data, off_obj_ext, 0x1234);

        // 1: process_table at 400 (highest concrete offset for DRC validation!)
        let off_proc_table = 400;
        set_ptr(&mut data, 1, off_proc_table);
        write_be16(&mut data, off_proc_table, base + 410);

        // Process 0 at 410: verb=1 (NORTH), noun=255 (_), condact at 420
        data[410] = 1;
        data[411] = 0xFF;
        write_be16(&mut data, 412, base + 420);
        data[414] = 0x00;

        // CondAct stream at 420: MESSAGE 0, DONE, 0xFF
        data[420] = 38; // MESSAGE
        data[421] = 0;
        data[422] = 22; // DONE
        data[423] = 0xFF;

        // 12: File length = 500
        set_ptr(&mut data, 12, 500);

        let payload = &data[..500];
        let doc = parse_ddb_ir_auto(payload, "synthetic.ddb").expect("should parse synthetic DDB");

        assert_eq!(doc.tokens.len(), 1);
        assert_eq!(doc.tokens[0].text, "room");
        assert_eq!(doc.vocabulary.len(), 3);
        assert_eq!(doc.messages[0].text, "Hello DAAD!");
        assert_eq!(doc.system_messages[0].text, "OK.");
        assert_eq!(doc.locations.len(), 2);
        assert_eq!(doc.locations[0].description, "A dark room.");
        assert_eq!(doc.locations[0].connections.len(), 1);
        assert_eq!(doc.locations[0].connections[0].verb_word.as_deref(), Some("NORTH"));
        assert_eq!(doc.locations[0].connections[0].destination, 1);

        assert_eq!(doc.objects.len(), 1);
        assert_eq!(doc.objects[0].description, "a brass lamp");
        assert_eq!(doc.objects[0].weight, 5);
        assert!(doc.objects[0].is_container);
        assert!(doc.objects[0].is_wearable);
        assert_eq!(doc.objects[0].extended_attributes, Some(0x1234));
        assert_eq!(doc.objects[0].noun_word.as_deref(), Some("LAMP"));
        assert_eq!(doc.objects[0].adjective_word.as_deref(), Some("BRASS"));

        assert_eq!(doc.processes.len(), 1);
        assert_eq!(doc.processes[0].entries.len(), 1);
        assert_eq!(doc.processes[0].entries[0].verb_word.as_deref(), Some("NORTH"));
        assert_eq!(doc.processes[0].entries[0].condacts.len(), 2);
        assert_eq!(doc.processes[0].entries[0].condacts[0].name, "MESSAGE");
        assert_eq!(doc.processes[0].entries[0].condacts[1].name, "DONE");
    }

    #[test]
    fn test_interpreter_fingerprinting() {
        let fake_bin = b"MZ\x90\x00some PCDAAD v2 runtime bytes";
        let report = fingerprint_binary(fake_bin, "custom_runner.exe");
        assert!(!report.is_ddb);
        let m = report.interpreter_match.expect("should match PCDAAD marker");
        assert_eq!(m.profile_id, "daad-dos-pcdaad-community");
        assert_eq!(m.match_kind, "marker");

        let report2 = fingerprint_binary(b"dummy z80 binary", "DS48IS.P3F");
        let m2 = report2.interpreter_match.expect("should match DS48IS filename");
        assert_eq!(m2.profile_id, "daad-zx-ds48is-official");
        assert_eq!(m2.confidence, "strong");
        assert_eq!(m2.language.as_deref(), Some("es"));
    }
}
