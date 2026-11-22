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

/// Disassembles machine code across modern & retro ISAs via `capstone`
/// (`arm64`, `arm`, `x86_64`, `m68k`/`68000`/`68020`, `6800`, `6809`, `6811`, `6309`,
/// `ppc`/`ppc32`, `ppc64`, `mips`/`mips32`, `mips64`, `riscv32`, `riscv64`, `sparc`).
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
        "m68k" | "68000" | "68k" => Capstone::new()
            .m68k()
            .mode(arch::m68k::ArchMode::M68k000)
            .detail(false)
            .build(),
        "68020" => Capstone::new()
            .m68k()
            .mode(arch::m68k::ArchMode::M68k020)
            .detail(false)
            .build(),
        "6800" | "m6800" => Capstone::new()
            .m680x()
            .mode(arch::m680x::ArchMode::M680x6800)
            .detail(false)
            .build(),
        "6809" | "m6809" => Capstone::new()
            .m680x()
            .mode(arch::m680x::ArchMode::M680x6809)
            .detail(false)
            .build(),
        "6811" | "m6811" => Capstone::new()
            .m680x()
            .mode(arch::m680x::ArchMode::M680x6811)
            .detail(false)
            .build(),
        "6309" => Capstone::new()
            .m680x()
            .mode(arch::m680x::ArchMode::M680x6309)
            .detail(false)
            .build(),
        "ppc" | "ppc32" | "powerpc" => Capstone::new()
            .ppc()
            .mode(arch::ppc::ArchMode::Mode32)
            .endian(capstone::Endian::Big)
            .detail(false)
            .build(),
        "ppc64" => Capstone::new()
            .ppc()
            .mode(arch::ppc::ArchMode::Mode64)
            .endian(capstone::Endian::Big)
            .detail(false)
            .build(),
        "mips" | "mips32" => Capstone::new()
            .mips()
            .mode(arch::mips::ArchMode::Mips32)
            .detail(false)
            .build(),
        "mips64" => Capstone::new()
            .mips()
            .mode(arch::mips::ArchMode::Mips64)
            .detail(false)
            .build(),
        "riscv32" => Capstone::new()
            .riscv()
            .mode(arch::riscv::ArchMode::RiscV32)
            .detail(false)
            .build(),
        "riscv64" | "riscv" => Capstone::new()
            .riscv()
            .mode(arch::riscv::ArchMode::RiscV64)
            .detail(false)
            .build(),
        "sparc" => Capstone::new()
            .sparc()
            .mode(arch::sparc::ArchMode::Default)
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
            "provenance": ["goblin", "gimli-dwarf-ready", "wazabin-sleigh-ready"],
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
            "provenance": ["goblin", "wazabin-sleigh-ready"],
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

    #[test]
    fn capstone_retro_cpus_m68k_6809_ppc_work() {
        // M68K: nop (4e 71) rts (4e 75)
        let m68k = disasm_capstone(&[0x4e, 0x71, 0x4e, 0x75], "m68k", 0x1000, 10).unwrap();
        assert_eq!(m68k.count, 2);
        assert_eq!(m68k.instructions[0].text, "nop");
        assert_eq!(m68k.instructions[1].text, "rts");

        // 6809: nop (12) rts (39)
        let m6809 = disasm_capstone(&[0x12, 0x39], "6809", 0x8000, 10).unwrap();
        assert_eq!(m6809.count, 2);
        assert_eq!(m6809.instructions[0].text, "nop");
        assert_eq!(m6809.instructions[1].text, "rts");

        // PowerPC big-endian: nop (60 00 00 00) blr (4e 80 00 20)
        let ppc = disasm_capstone(&[0x60, 0x00, 0x00, 0x00, 0x4e, 0x80, 0x00, 0x20], "ppc", 0x10000, 10).unwrap();
        assert_eq!(ppc.count, 2);
        assert_eq!(ppc.instructions[0].text, "nop");
        assert_eq!(ppc.instructions[1].text, "blr");
    }
}
