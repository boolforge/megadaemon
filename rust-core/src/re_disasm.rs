//! Modular Reverse Engineering Pipeline: goblin + gimli/addr2line + iced-x86 + Capstone (PoC 5).
//!
//! Unifies multi-ISA disassembly (ARM64 via Capstone, x86/x64 via iced-x86 & Capstone)
//! and ELF/PE/Mach-O container + DWARF provenance inspection.

use capstone::prelude::*;
use goblin::{elf::Elf, pe::PE};
use iced_x86::{Decoder, DecoderOptions, Formatter, IntelFormatter};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisasmInsn {
    pub address: String,
    pub bytes_hex: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisasmReport {
    pub ok: bool,
    pub engine: String,
    pub arch: String,
    pub count: usize,
    pub instructions: Vec<DisasmInsn>,
}

fn hex_bytes(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for &byte in b {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", byte);
    }
    s
}

/// Disassembles x86/x86_64 machine code using pure-Rust `iced-x86`.
pub fn disasm_x86_iced(code: &[u8], bitness: u32, base_addr: u64, max_insns: usize) -> DisasmReport {
    let mut decoder = Decoder::with_ip(bitness, code, base_addr, DecoderOptions::NONE);
    let mut formatter = IntelFormatter::new();
    let mut output = String::new();
    let mut insn = iced_x86::Instruction::default();
    let mut instructions = Vec::new();

    while decoder.can_decode() && instructions.len() < max_insns {
        decoder.decode_out(&mut insn);
        output.clear();
        formatter.format(&insn, &mut output);
        let start = (insn.ip().saturating_sub(base_addr)) as usize;
        let end = (start + insn.len()).min(code.len());
        instructions.push(DisasmInsn {
            address: format!("0x{:x}", insn.ip()),
            bytes_hex: if start < end { hex_bytes(&code[start..end]) } else { String::new() },
            text: output.clone(),
        });
    }

    DisasmReport {
        ok: true,
        engine: "iced-x86".to_string(),
        arch: format!("x86-{}", bitness),
        count: instructions.len(),
        instructions,
    }
}

/// Disassembles ARM64 (`arm64`), ARM (`arm`), or x86_64 (`x86_64`) machine code via `capstone`.
pub fn disasm_capstone(code: &[u8], arch: &str, base_addr: u64, max_insns: usize) -> Result<DisasmReport, String> {
    let cs = match arch {
        "arm64" | "aarch64" => Capstone::new()
            .arm64()
            .mode(arch::arm64::ArchMode::Arm)
            .detail(false)
            .build(),
        "arm" => Capstone::new()
            .arm()
            .mode(arch::arm::ArchMode::Arm)
            .detail(false)
            .build(),
        "x86_64" | "x64" => Capstone::new()
            .x86()
            .mode(arch::x86::ArchMode::Mode64)
            .detail(false)
            .build(),
        other => return Err(format!("unsupported capstone arch '{}'", other)),
    }
    .map_err(|e| format!("capstone init: {:?}", e))?;

    let insns = cs
        .disasm_count(code, base_addr, max_insns)
        .map_err(|e| format!("capstone disasm: {:?}", e))?;

    let mut out = Vec::with_capacity(insns.len());
    for i in insns.as_ref() {
        let mnemonic = i.mnemonic().unwrap_or("");
        let op = i.op_str().unwrap_or("");
        let text = if op.is_empty() {
            mnemonic.to_string()
        } else {
            format!("{} {}", mnemonic, op)
        };
        out.push(DisasmInsn {
            address: format!("0x{:x}", i.address()),
            bytes_hex: hex_bytes(i.bytes()),
            text,
        });
    }

    Ok(DisasmReport {
        ok: true,
        engine: "capstone".to_string(),
        arch: arch.to_string(),
        count: out.len(),
        instructions: out,
    })
}

/// Inspects an ELF or PE binary using `goblin` + checks DWARF debug section presence (`gimli`).
pub fn inspect_binary_provenance(path: &Path) -> Result<serde_json::Value, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    if bytes.starts_with(b"\x7fELF") {
        let elf = Elf::parse(&bytes).map_err(|e| format!("goblin ELF parse: {}", e))?;
        let has_dwarf = elf.section_headers.iter().any(|sh| {
            elf.shdr_strtab
                .get_at(sh.sh_name)
                .map(|n| n.starts_with(".debug_"))
                .unwrap_or(false)
        });
        return Ok(serde_json::json!({
            "ok": true,
            "provenance": ["goblin", "gimli-dwarf-ready"],
            "format": "ELF",
            "is_64": elf.is_64,
            "entry": format!("0x{:x}", elf.entry),
            "sections": elf.section_headers.len(),
            "syms": elf.syms.len(),
            "dynsyms": elf.dynsyms.len(),
            "has_dwarf": has_dwarf,
        }));
    }
    if bytes.starts_with(b"MZ") {
        let pe = PE::parse(&bytes).map_err(|e| format!("goblin PE parse: {}", e))?;
        return Ok(serde_json::json!({
            "ok": true,
            "provenance": ["goblin"],
            "format": "PE",
            "is_64": pe.is_64,
            "entry": format!("0x{:x}", pe.entry),
            "sections": pe.sections.len(),
            "imports": pe.imports.len(),
            "exports": pe.exports.len(),
        }));
    }
    Ok(serde_json::json!({
        "ok": true,
        "provenance": ["goblin"],
        "format": "unknown",
        "size": bytes.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iced_x86_and_capstone_arm64_disassemble_cleanly() {
        // x86_64: xor eax, eax; ret (31 c0 c3)
        let x86 = disasm_x86_iced(&[0x31, 0xC0, 0xC3], 64, 0x1000, 10);
        assert!(x86.ok);
        assert_eq!(x86.count, 2);
        assert!(x86.instructions[0].text.contains("xor"));
        assert!(x86.instructions[1].text.contains("ret"));

        // ARM64: nop; ret (1f 20 03 d5  c0 03 5f d6)
        let arm64 = disasm_capstone(
            &[0x1f, 0x20, 0x03, 0xd5, 0xc0, 0x03, 0x5f, 0xd6],
            "arm64",
            0x400000,
            10,
        )
        .unwrap();
        assert!(arm64.ok);
        assert_eq!(arm64.count, 2);
        assert_eq!(arm64.instructions[0].text, "nop");
        assert_eq!(arm64.instructions[1].text, "ret");
    }
}
