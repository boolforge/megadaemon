//! Pure-Rust ScummVM Savegame Header & Compression Inspector.

use serde_json::{json, Value};

/// Parses a ScummVM `.s00-.s99` savegame header (`SCVM` magic + big-endian version + compression hint).
pub fn parse_scummvm_savegame(data: &[u8]) -> Value {
    if data.len() < 16 {
        return json!({ "ok": false, "error": "save buffer too small (<16 bytes)" });
    }
    if &data[..4] != b"SCVM" {
        return json!({
            "ok": false,
            "valid_scummvm": false,
            "raw_magic": format!("{:02x}{:02x}{:02x}{:02x}", data[0], data[1], data[2], data[3]),
        });
    }
    let ver = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let payload = &data[8..];
    let compression = if payload.len() >= 2 && payload[0] == 0x1f && payload[1] == 0x8b {
        "gzip"
    } else if payload.len() >= 2 && payload[0] == 0x78 {
        "zlib"
    } else {
        "raw"
    };
    json!({
        "ok": true,
        "valid_scummvm": true,
        "format_version": ver,
        "compression_hint": compression,
        "header_len": 16,
        "payload_size": data.len() - 16,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scummvm_header() {
        let mut save = b"SCVM\x00\x00\x00\x05".to_vec();
        save.extend_from_slice(&[0u8; 24]);
        let sc = parse_scummvm_savegame(&save);
        assert_eq!(sc["valid_scummvm"], true);
        assert_eq!(sc["format_version"], 5);
        assert_eq!(sc["payload_size"], 16);
    }
}
