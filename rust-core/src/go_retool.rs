//! MegaDaemon Go Reverse Engineering Bridge (`go-retool`).
//!
//! Integrates the pure-Rust `go-retool` crate (`scan_container`, `parse_go118_plus`,
//! `decode_pcvalue`, `decode_stackmap`, `decode_embed_records`, `walk_module_chain`)
//! with automated `.text` base discovery, package clustering, Ghidra JSON export,
//! and MegaDaemon MCP/CLI tool dispatch.

use go_retool::container::{scan_container, ContainerReport};
use go_retool::model::{Analysis, Function};
use go_retool::pclntab::{parse_go118_plus, ParseOptions};
use go_retool::stackmap::{decode_stackmap, StackMap};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSummary {
    pub package: String,
    pub function_count: usize,
    pub sample_functions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoBinaryReport {
    pub ok: bool,
    pub path: String,
    pub size_bytes: usize,
    pub format: String,
    pub architecture: String,
    pub endianness: String,
    pub is_64: bool,
    pub section_count: usize,
    pub pclntab_candidate_count: usize,
    pub selected_pclntab_offset: Option<String>,
    pub selected_text_base: Option<String>,
    pub go_version: Option<String>,
    pub total_functions: usize,
    pub named_functions: usize,
    pub packages: Vec<PackageSummary>,
    pub main_functions: Vec<String>,
    pub warnings: Vec<String>,
}

fn parse_hex_u64(s: &str) -> Option<u64> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        t.parse::<u64>().ok()
    }
}

fn extract_package_name(sym: &str) -> String {
    // Go symbols typically look like `main.run`, `net/http.(*Client).Do`, `runtime.mallocgc`.
    let before_paren = sym.split('(').next().unwrap_or(sym);
    if let Some(idx) = before_paren.rfind('.') {
        let pkg = &before_paren[..idx];
        if !pkg.is_empty() {
            return pkg.to_string();
        }
    }
    "<root>".to_string()
}

/// Scans an executable container and selects the best `.gopclntab` + `.text` base candidate.
pub fn scan_and_parse(data: &[u8]) -> Result<(ContainerReport, Option<Analysis>), String> {
    let container = scan_container(data).map_err(|e| e.to_string())?;
    let best = container
        .pclntab_candidates
        .iter()
        .max_by_key(|c| (c.names_resolved, c.nfunc));

    let Some(cand) = best else {
        return Ok((container, None));
    };

    let file_offset = parse_hex_u64(&cand.file_offset)
        .ok_or_else(|| format!("invalid pclntab file_offset: {}", cand.file_offset))?;

    // Best-effort text base from the first `.text` / `__text` section for quick VA mapping;
    // `go-retool` explicitly annotates its status when unverified.
    let text_base = container
        .sections
        .iter()
        .find(|s| s.kind == "text" || s.name == ".text" || s.name == "__text")
        .and_then(|s| parse_hex_u64(&s.section_address));

    let opts = ParseOptions {
        file_offset,
        text_base,
        max_functions: 250_000,
        ..Default::default()
    };
    let analysis = parse_go118_plus(data, opts).map_err(|e| e.to_string())?;
    Ok((container, Some(analysis)))
}

/// Comprehensive high-level analysis of a Go executable on disk.
pub fn analyze_go_file(path: &Path) -> Result<GoBinaryReport, String> {
    let data = fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    let (container, maybe_analysis) = scan_and_parse(&data)?;

    let mut pkg_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut main_functions = Vec::new();
    let mut selected_pclntab_offset = None;
    let mut selected_text_base = None;
    let mut go_version = None;
    let mut total_functions = 0usize;
    let mut named_functions = 0usize;
    let mut warnings = container.warnings.clone();

    if let Some(analysis) = &maybe_analysis {
        warnings.extend(analysis.warnings.iter().cloned());
        if let Some(m) = analysis.modules.first() {
            selected_pclntab_offset = Some(m.pclntab_file_offset.clone());
            selected_text_base = m.text_base_va.clone();
            go_version = m.go_version.clone();
            total_functions = m.functions.len();
            warnings.extend(m.warnings.iter().cloned());

            for f in &m.functions {
                if let Some(name) = &f.observed_name {
                    named_functions += 1;
                    let pkg = extract_package_name(name);
                    pkg_map.entry(pkg.clone()).or_default().push(name.clone());
                    if pkg == "main" && main_functions.len() < 128 {
                        main_functions.push(name.clone());
                    }
                }
            }
        }
    }

    let mut packages: Vec<PackageSummary> = pkg_map
        .into_iter()
        .map(|(package, mut fns)| {
            let count = fns.len();
            fns.truncate(8);
            PackageSummary {
                package,
                function_count: count,
                sample_functions: fns,
            }
        })
        .collect();
    packages.sort_by(|a, b| b.function_count.cmp(&a.function_count));
    packages.truncate(50);

    Ok(GoBinaryReport {
        ok: true,
        path: path.display().to_string(),
        size_bytes: data.len(),
        format: container.format,
        architecture: container.architecture,
        endianness: container.endianness,
        is_64: container.is_64,
        section_count: container.sections.len(),
        pclntab_candidate_count: container.pclntab_candidates.len(),
        selected_pclntab_offset,
        selected_text_base,
        go_version,
        total_functions,
        named_functions,
        packages,
        main_functions,
        warnings,
    })
}

/// Extracts filtered Go functions (by substring) for targeted RE inspection.
pub fn query_functions(path: &Path, filter: Option<&str>, limit: usize) -> Result<Vec<Function>, String> {
    let data = fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    let (_, maybe_analysis) = scan_and_parse(&data)?;
    let Some(analysis) = maybe_analysis else {
        return Err("no Go 1.18+ pclntab found in binary".to_string());
    };
    let needle = filter.unwrap_or("").trim().to_ascii_lowercase();
    let cap = limit.clamp(1, 5000);
    let mut out = Vec::new();
    for m in analysis.modules {
        for f in m.functions {
            if !needle.is_empty() {
                let hit = f
                    .observed_name
                    .as_deref()
                    .map(|n| n.to_ascii_lowercase().contains(&needle))
                    .unwrap_or(false);
                if !hit {
                    continue;
                }
            }
            out.push(f);
            if out.len() >= cap {
                return Ok(out);
            }
        }
    }
    Ok(out)
}

/// Produces full `go-retool` `Analysis` JSON compatible with `ImportGoRetoolJson.py` for Ghidra.
pub fn export_ghidra_json(path: &Path) -> Result<Value, String> {
    let data = fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    let (_, maybe_analysis) = scan_and_parse(&data)?;
    let analysis = maybe_analysis.ok_or_else(|| "no Go 1.18+ pclntab found".to_string())?;
    serde_json::to_value(&analysis).map_err(|e| e.to_string())
}

/// Decodes a Go stackmap slice at a given file offset.
pub fn inspect_stackmap_at(path: &Path, offset: usize) -> Result<StackMap, String> {
    let data = fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    if offset >= data.len() {
        return Err(format!("offset 0x{:x} out of bounds (size={})", offset, data.len()));
    }
    decode_stackmap(&data[offset..], 4096, 4096).map_err(|e| e.to_string())
}

/// Unified dispatcher for MCP & CLI.
pub fn handle_action(action: &str, path: &Path, query: Option<&str>, limit: usize) -> Value {
    match action {
        "container" => match fs::read(path).and_then(|d| {
            scan_container(&d).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
        }) {
            Ok(rep) => json!({ "ok": true, "container": rep }),
            Err(e) => json!({ "ok": false, "error": e.to_string() }),
        },
        "functions" => match query_functions(path, query, limit) {
            Ok(fns) => json!({ "ok": true, "count": fns.len(), "functions": fns }),
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "ghidra_json" => match export_ghidra_json(path) {
            Ok(v) => v,
            Err(e) => json!({ "ok": false, "error": e }),
        },
        _ => match analyze_go_file(path) {
            Ok(rep) => serde_json::to_value(&rep).unwrap_or_else(|e| json!({ "ok": false, "error": e.to_string() })),
            Err(e) => json!({ "ok": false, "error": e }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_go_package_names_accurately() {
        assert_eq!(extract_package_name("main.runLoop"), "main");
        assert_eq!(extract_package_name("net/http.(*Client).Do"), "net/http");
        assert_eq!(extract_package_name("runtime.mallocgc"), "runtime");
    }

    #[test]
    fn rejects_non_executable_cleanly() {
        let res = scan_and_parse(b"not a valid elf or pe executable");
        assert!(res.is_err());
    }
}
