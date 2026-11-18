use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PatternMatch {
    pub offset: usize,
    pub matched_hex: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatternResult {
    pub pattern: String,
    pub total_matches: usize,
    pub matches: Vec<PatternMatch>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PatternByte {
    Exact(u8),
    Wildcard,
}

pub fn parse_aob_pattern(pat_str: &str) -> Result<Vec<PatternByte>, String> {
    let mut tokens = Vec::new();
    for token in pat_str.split_whitespace() {
        if token == "??" || token == "?" || token == "*" {
            tokens.push(PatternByte::Wildcard);
        } else {
            let b = u8::from_str_radix(token, 16)
                .map_err(|e| format!("Invalid hex byte '{}': {}", token, e))?;
            tokens.push(PatternByte::Exact(b));
        }
    }
    if tokens.is_empty() {
        return Err("Pattern cannot be empty".to_string());
    }
    Ok(tokens)
}

pub fn scan_pattern(data: &[u8], pat_str: &str, max_matches: usize) -> Result<PatternResult, String> {
    let pattern = parse_aob_pattern(pat_str)?;
    let pat_len = pattern.len();
    if pat_len > data.len() {
        return Ok(PatternResult {
            pattern: pat_str.to_string(),
            total_matches: 0,
            matches: vec![],
        });
    }

    let limit = if max_matches == 0 { 256 } else { max_matches };
    let mut matches = Vec::new();
    let max_idx = data.len() - pat_len;

    for i in 0..=max_idx {
        let mut matched = true;
        for j in 0..pat_len {
            match pattern[j] {
                PatternByte::Exact(expected) => {
                    if data[i + j] != expected {
                        matched = false;
                        break;
                    }
                }
                PatternByte::Wildcard => {
                    // Always matches
                }
            }
        }

        if matched {
            let hex_slice = data[i..i + pat_len]
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");

            matches.push(PatternMatch {
                offset: i,
                matched_hex: hex_slice,
            });

            if matches.len() >= limit {
                break;
            }
        }
    }

    let total = matches.len();
    Ok(PatternResult {
        pattern: pat_str.to_string(),
        total_matches: total,
        matches,
    })
}
