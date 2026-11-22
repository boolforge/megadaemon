//! Structural Code Intelligence via Tree-sitter (PoC 2).
//!
//! Provides exact AST symbol extraction (fn, struct, enum, trait, impl, mod)
//! and post-edit syntax error detection for Rust source files.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstSymbol {
    pub kind: String,
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstSyntaxError {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstReport {
    pub ok: bool,
    pub language: String,
    pub symbol_count: usize,
    pub error_count: usize,
    pub symbols: Vec<AstSymbol>,
    pub errors: Vec<AstSyntaxError>,
}

fn collect_symbols_and_errors(node: Node<'_>, src: &[u8], symbols: &mut Vec<AstSymbol>, errors: &mut Vec<AstSyntaxError>) {
    if node.is_error() || node.is_missing() {
        let start = node.start_position();
        let end = node.end_position();
        let raw = node.utf8_text(src).unwrap_or("").trim();
        let snippet: String = raw.chars().take(80).collect();
        errors.push(AstSyntaxError {
            start_line: start.row + 1,
            start_col: start.column + 1,
            end_line: end.row + 1,
            snippet,
        });
    }

    let kind = node.kind();
    let interesting = matches!(
        kind,
        "function_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "impl_item"
            | "mod_item"
            | "const_item"
            | "static_item"
    );
    if interesting {
        let name = node
            .child_by_field_name("name")
            .or_else(|| node.child_by_field_name("type"))
            .and_then(|n| n.utf8_text(src).ok())
            .unwrap_or("<anonymous>")
            .to_string();
        symbols.push(AstSymbol {
            kind: kind.trim_end_matches("_item").to_string(),
            name,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols_and_errors(child, src, symbols, errors);
    }
}

/// Parses Rust source text with `tree-sitter-rust` and returns symbols + syntax errors.
pub fn analyze_rust_source(source: &str) -> Result<AstReport, String> {
    let mut parser = Parser::new();
    let lang = tree_sitter_rust::LANGUAGE;
    parser
        .set_language(&lang.into())
        .map_err(|e| format!("tree-sitter set_language: {:?}", e))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "tree-sitter failed to produce syntax tree".to_string())?;
    let mut symbols = Vec::new();
    let mut errors = Vec::new();
    collect_symbols_and_errors(tree.root_node(), source.as_bytes(), &mut symbols, &mut errors);
    Ok(AstReport {
        ok: errors.is_empty(),
        language: "rust".to_string(),
        symbol_count: symbols.len(),
        error_count: errors.len(),
        symbols,
        errors,
    })
}

/// Analyzes a file on disk if it is a supported Tree-sitter language (currently `.rs`).
pub fn analyze_file(path: &Path) -> Result<AstReport, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext != "rs" {
        return Err(format!("unsupported AST extension '.{}' (supported: .rs)", ext));
    }
    let src = std::fs::read_to_string(path)
        .map_err(|e| format!("read {}: {}", path.display(), e))?;
    analyze_rust_source(&src)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_rust_symbols_and_detects_syntax_errors() {
        let valid = r#"
            pub struct Engine { port: u16 }
            impl Engine {
                pub fn start(&self) -> u16 { self.port }
            }
        "#;
        let rep = analyze_rust_source(valid).unwrap();
        assert!(rep.ok);
        assert_eq!(rep.error_count, 0);
        assert!(rep.symbols.iter().any(|s| s.kind == "struct" && s.name == "Engine"));
        assert!(rep.symbols.iter().any(|s| s.kind == "function" && s.name == "start"));

        let broken = "pub fn broken( { let x = ; }";
        let bad = analyze_rust_source(broken).unwrap();
        assert!(!bad.ok);
        assert!(bad.error_count > 0);
    }
}
