use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryDelta {
    pub offset: usize,
    pub address_hex: String,
    pub old_val: u8,
    pub new_val: u8,
    pub mutation_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotDiffResult {
    pub buffer_a_size: usize,
    pub buffer_b_size: usize,
    pub total_mutations: usize,
    pub flag_sets: usize,
    pub flag_clears: usize,
    pub counter_increments: usize,
    pub counter_decrements: usize,
    pub deltas: Vec<MemoryDelta>,
}

pub fn diff_snapshots(
    buf_a: &[u8],
    buf_b: &[u8],
    base_addr: u32,
    max_deltas: usize,
) -> SnapshotDiffResult {
    let min_len = buf_a.len().min(buf_b.len());
    let limit = if max_deltas == 0 { 256 } else { max_deltas };

    let mut deltas = Vec::new();
    let mut flag_sets = 0;
    let mut flag_clears = 0;
    let mut counter_increments = 0;
    let mut counter_decrements = 0;

    let mut i = 0;
    // Fast path: scan 8-byte (64-bit) chunks
    while i + 8 <= min_len {
        let w_a = u64::from_ne_bytes(buf_a[i..i + 8].try_into().unwrap());
        let w_b = u64::from_ne_bytes(buf_b[i..i + 8].try_into().unwrap());
        if w_a != w_b {
            // Find which bytes differ in this chunk
            for byte_idx in 0..8 {
                let off = i + byte_idx;
                let b_a = buf_a[off];
                let b_b = buf_b[off];
                if b_a != b_b {
                    let mutation = if b_a == 0 && b_b == 1 {
                        flag_sets += 1;
                        "FLAG_SET"
                    } else if b_a == 1 && b_b == 0 {
                        flag_clears += 1;
                        "FLAG_CLEARED"
                    } else if b_b == b_a.wrapping_add(1) {
                        counter_increments += 1;
                        "COUNTER_INCREMENT"
                    } else if b_b == b_a.wrapping_sub(1) {
                        counter_decrements += 1;
                        "COUNTER_DECREMENT"
                    } else {
                        "VALUE_TRANSITION"
                    };

                    if deltas.len() < limit {
                        deltas.push(MemoryDelta {
                            offset: off,
                            address_hex: format!("0x{:08X}", base_addr + off as u32),
                            old_val: b_a,
                            new_val: b_b,
                            mutation_type: mutation.to_string(),
                        });
                    }
                }
            }
        }
        i += 8;
    }

    // Remainder bytes
    while i < min_len {
        let b_a = buf_a[i];
        let b_b = buf_b[i];
        if b_a != b_b {
            let mutation = if b_a == 0 && b_b == 1 {
                flag_sets += 1;
                "FLAG_SET"
            } else if b_a == 1 && b_b == 0 {
                flag_clears += 1;
                "FLAG_CLEARED"
            } else if b_b == b_a.wrapping_add(1) {
                counter_increments += 1;
                "COUNTER_INCREMENT"
            } else if b_b == b_a.wrapping_sub(1) {
                counter_decrements += 1;
                "COUNTER_DECREMENT"
            } else {
                "VALUE_TRANSITION"
            };

            if deltas.len() < limit {
                deltas.push(MemoryDelta {
                    offset: i,
                    address_hex: format!("0x{:08X}", base_addr + i as u32),
                    old_val: b_a,
                    new_val: b_b,
                    mutation_type: mutation.to_string(),
                });
            }
        }
        i += 1;
    }

    let total = flag_sets + flag_clears + counter_increments + counter_decrements + (deltas.len() - (flag_sets + flag_clears + counter_increments + counter_decrements).min(deltas.len()));

    SnapshotDiffResult {
        buffer_a_size: buf_a.len(),
        buffer_b_size: buf_b.len(),
        total_mutations: total,
        flag_sets,
        flag_clears,
        counter_increments,
        counter_decrements,
        deltas,
    }
}
