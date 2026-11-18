use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct EntropyBlock {
    pub offset: usize,
    pub size: usize,
    pub entropy: f64,
    pub classification: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntropyResult {
    pub file_size: usize,
    pub global_entropy: f64,
    pub block_size: usize,
    pub total_blocks: usize,
    pub classifications_summary: SummaryCounts,
    pub blocks: Vec<EntropyBlock>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SummaryCounts {
    pub padding_null: usize,
    pub struct_pointer_table: usize,
    pub code_or_text: usize,
    pub planar_gfx_or_pcm: usize,
    pub compressed_or_encrypted: usize,
}

#[inline]
pub fn calculate_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut freq = [0usize; 256];
    for &b in data {
        freq[b as usize] += 1;
    }
    let len_f = data.len() as f64;
    let mut ent = 0.0;
    for &count in &freq {
        if count > 0 {
            let p = count as f64 / len_f;
            ent -= p * p.log2();
        }
    }
    ent
}

#[inline]
pub fn classify_entropy(ent: f64) -> &'static str {
    if ent < 0.5 {
        "PADDING_NULL"
    } else if ent < 3.5 {
        "STRUCT_POINTER_TABLE"
    } else if ent < 6.8 {
        "CODE_OR_TEXT"
    } else if ent < 7.4 {
        "PLANAR_GFX_OR_PCM"
    } else {
        "COMPRESSED_OR_ENCRYPTED"
    }
}

pub fn scan_entropy(data: &[u8], block_size: usize) -> EntropyResult {
    let bsize = if block_size == 0 { 1024 } else { block_size };
    let global_ent = calculate_entropy(data);
    let mut blocks = Vec::new();
    let mut summary = SummaryCounts::default();

    for (i, chunk) in data.chunks(bsize).enumerate() {
        let ent = calculate_entropy(chunk);
        let class = classify_entropy(ent);
        match class {
            "PADDING_NULL" => summary.padding_null += 1,
            "STRUCT_POINTER_TABLE" => summary.struct_pointer_table += 1,
            "CODE_OR_TEXT" => summary.code_or_text += 1,
            "PLANAR_GFX_OR_PCM" => summary.planar_gfx_or_pcm += 1,
            _ => summary.compressed_or_encrypted += 1,
        }

        // Limit block detail to max 128 blocks to keep JSON payload lean
        if blocks.len() < 128 {
            blocks.push(EntropyBlock {
                offset: i * bsize,
                size: chunk.len(),
                entropy: (ent * 1000.0).round() / 1000.0,
                classification: class.to_string(),
            });
        }
    }

    let total_blocks = (data.len() + bsize - 1) / bsize;
    EntropyResult {
        file_size: data.len(),
        global_entropy: (global_ent * 1000.0).round() / 1000.0,
        block_size: bsize,
        total_blocks,
        classifications_summary: summary,
        blocks,
    }
}
