use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PointerTableEntry {
    pub table_offset: usize,
    pub pointer_val: u32,
    pub target_offset: usize,
    pub sample_str: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PointerScanResult {
    pub file_size: usize,
    pub pointer_width: usize,
    pub is_big_endian: bool,
    pub total_tables_detected: usize,
    pub pointers_found: Vec<PointerTableEntry>,
}

fn extract_ascii_sjis(data: &[u8], offset: usize, max_len: usize) -> Option<String> {
    if offset >= data.len() {
        return None;
    }
    let mut end = offset;
    while end < data.len() && end - offset < max_len && data[end] != 0 {
        let b = data[end];
        // Printable ASCII or Shift-JIS lead bytes
        if (0x20..=0x7E).contains(&b) || (0x81..=0x9F).contains(&b) || (0xE0..=0xEF).contains(&b) {
            end += 1;
        } else {
            break;
        }
    }
    if end - offset >= 3 {
        Some(String::from_utf8_lossy(&data[offset..end]).to_string())
    } else {
        None
    }
}

pub fn scan_pointers_32(
    data: &[u8],
    base_vaddr: u32,
    is_big_endian: bool,
    max_results: usize,
) -> PointerScanResult {
    let mut results = Vec::new();
    let limit = if max_results == 0 { 128 } else { max_results };
    let file_len = data.len();

    if file_len < 4 {
        return PointerScanResult {
            file_size: file_len,
            pointer_width: 32,
            is_big_endian,
            total_tables_detected: 0,
            pointers_found: vec![],
        };
    }

    let mut i = 0;
    while i <= file_len - 4 {
        let ptr_val = if is_big_endian {
            u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]])
        } else {
            u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]])
        };

        // Check if pointer falls within binary image relative to base_vaddr
        if ptr_val >= base_vaddr && (ptr_val - base_vaddr) < file_len as u32 {
            let target_off = (ptr_val - base_vaddr) as usize;
            if target_off != i {
                if let Some(s) = extract_ascii_sjis(data, target_off, 48) {
                    results.push(PointerTableEntry {
                        table_offset: i,
                        pointer_val: ptr_val,
                        target_offset: target_off,
                        sample_str: s,
                    });
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }
        i += 4;
    }

    let total = results.len();
    PointerScanResult {
        file_size: file_len,
        pointer_width: 32,
        is_big_endian,
        total_tables_detected: if total > 4 { 1 } else { 0 },
        pointers_found: results,
    }
}
