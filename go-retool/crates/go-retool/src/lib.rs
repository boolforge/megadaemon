//! Conservative Go pclntab reader. It parses the modern Go 1.18+ functab
//! representation (two u32 fields per record) and intentionally does not
//! pretend to recover moduledata or absolute VAs without an explicit mapping.
#![forbid(unsafe_code)]

pub mod address;
pub mod container;
pub mod embed;
pub mod error;
pub mod model;
pub mod module_chain;
pub mod pctab;
pub mod pclntab;
pub mod reader;
pub mod stackmap;

pub use error::{Error, Result};
pub use pclntab::{parse_go118_plus, ParseOptions};
