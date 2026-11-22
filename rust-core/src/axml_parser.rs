//! Pure-Rust Android Binary XML (`AXML` / `AndroidManifest.xml`) Decoder.
//!
//! Decodes `RES_STRING_POOL_TYPE` (UTF-8 and UTF-16LE) and `RES_XML_START_ELEMENT_TYPE`
//! into structured elements and attributes without Python or `aapt2`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxmlElement {
    pub name: String,
    pub depth: usize,
    pub attributes: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxmlReport {
    pub ok: bool,
    pub strings_count: usize,
    pub elements_count: usize,
    pub elements: Vec<AxmlElement>,
}

fn u16_le(d: &[u8], off: usize) -> Result<u16, String> {
    let s = d.get(off..off + 2).ok_or("AXML truncated u16")?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_le(d: &[u8], off: usize) -> Result<u32, String> {
    let s = d.get(off..off + 4).ok_or("AXML truncated u32")?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Decodes Android Binary XML (`\x03\x00\x08\x00` AXML) into structured elements and attributes.
pub fn parse_axml(data: &[u8]) -> Result<AxmlReport, String> {
    if data.len() < 16 || &data[..4] != b"\x03\x00\x08\x00" {
        return Err("invalid AXML magic (expected 03 00 08 00)".to_string());
    }
    let mut pos = 8usize;
    let sp_type = u16_le(data, pos)?;
    if sp_type != 0x0001 {
        return Err(format!("expected RES_STRING_POOL_TYPE (0x0001), got 0x{:04x}", sp_type));
    }
    let sp_size = u32_le(data, pos + 4)? as usize;
    let str_count = u32_le(data, pos + 8)? as usize;
    let flags = u32_le(data, pos + 16)?;
    let str_start = u32_le(data, pos + 20)? as usize;
    let is_utf8 = (flags & (1 << 8)) != 0;
    let base = pos + str_start;

    let mut strings = Vec::with_capacity(str_count.min(65536));
    for i in 0..str_count {
        let o = u32_le(data, pos + 28 + 4 * i)? as usize;
        let mut p = base + o;
        if is_utf8 {
            let n1 = *data.get(p).ok_or("AXML utf8 len1 oob")?;
            p += 1;
            if (n1 & 0x80) != 0 {
                p += 1;
            }
            let b1 = *data.get(p).ok_or("AXML utf8 blen1 oob")? as usize;
            p += 1;
            let bl = if (b1 & 0x80) != 0 {
                let b2 = *data.get(p).ok_or("AXML utf8 blen2 oob")? as usize;
                p += 1;
                ((b1 & 0x7F) << 8) | b2
            } else {
                b1
            };
            let slice = data.get(p..p + bl).ok_or("AXML utf8 slice oob")?;
            strings.push(String::from_utf8_lossy(slice).to_string());
        } else {
            let ln = u16_le(data, p)? as usize;
            p += 2;
            let mut units = Vec::with_capacity(ln);
            for j in 0..ln {
                units.push(u16_le(data, p + j * 2)?);
            }
            strings.push(String::from_utf16_lossy(&units));
        }
    }
    pos += sp_size;

    let get_str = |idx: u32| -> String {
        if idx == 0xFFFF_FFFF {
            String::new()
        } else {
            strings.get(idx as usize).cloned().unwrap_or_default()
        }
    };

    let mut elements = Vec::new();
    let mut depth = 0usize;
    while pos + 8 <= data.len() {
        let ctype = u16_le(data, pos)?;
        let csize = u32_le(data, pos + 4)? as usize;
        if csize < 8 || pos + csize > data.len() {
            break;
        }
        if ctype == 0x0102 && pos + 36 <= data.len() {
            let name_idx = u32_le(data, pos + 20)?;
            let attr_count = u16_le(data, pos + 28)? as usize;
            let mut attrs = Vec::with_capacity(attr_count);
            let mut ap = pos + 36;
            for _ in 0..attr_count {
                if ap + 20 > pos + csize {
                    break;
                }
                let aname = get_str(u32_le(data, ap + 4)?);
                let raw_val = u32_le(data, ap + 8)?;
                let vtype = data[ap + 15];
                let vdata = u32_le(data, ap + 16)?;
                let val_str = if raw_val != 0xFFFF_FFFF {
                    get_str(raw_val)
                } else if vtype == 0x03 {
                    get_str(vdata)
                } else if vtype == 0x12 {
                    (vdata != 0).to_string()
                } else {
                    format!("0x{:x}", vdata)
                };
                attrs.push((aname, val_str));
                ap += 20;
            }
            elements.push(AxmlElement {
                name: get_str(name_idx),
                depth,
                attributes: attrs,
            });
            depth += 1;
        } else if ctype == 0x0103 {
            depth = depth.saturating_sub(1);
        }
        pos += csize;
    }

    Ok(AxmlReport {
        ok: true,
        strings_count: strings.len(),
        elements_count: elements.len(),
        elements,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_axml_header() {
        assert!(parse_axml(b"<manifest/>").is_err());
    }
}
