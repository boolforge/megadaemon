//! Conservative Go pclntab reader. It parses the modern Go 1.18+ functab
//! representation (two u32 fields per record) and intentionally does not
//! pretend to recover moduledata or absolute VAs without an explicit mapping.
#![forbid(unsafe_code)]

pub mod address;
pub mod container;
pub mod embed;
pub mod error;
pub mod filetab;
pub mod model;
pub mod module_chain;
pub mod pctab;
pub mod pclntab;
pub mod reader;
pub mod sha256;
pub mod stackmap;
pub mod symbols;

pub use error::{Error, Result};
pub use pclntab::{parse_go118_plus, ParseOptions};
pub use symbols::{build_export, parse_json, parse_tsv, to_rizin_script, to_tsv, SymbolExport};
