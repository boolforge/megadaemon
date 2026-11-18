use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisasmInsn {
    pub addr: String,
    pub bytes: String,
    pub mnemonic: String,
    pub op_str: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisasmReport {
    pub platform: String,
    pub arch: String,
    pub file: String,
    pub offset: u64,
    pub instructions: Vec<DisasmInsn>,
    pub engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexLine {
    pub addr: String,
    pub hex: String,
    pub ascii: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexDumpReport {
    pub file: String,
    pub arch: String,
    pub offset: u64,
    pub total_size: u64,
    pub lines: Vec<HexLine>,
    pub engine: String,
}

pub fn inspect_hex(file_path: &str, arch: &str, offset: u64, length: usize) -> Result<HexDumpReport, String> {
    let mut f = File::open(file_path).map_err(|e| format!("Failed to open {}: {}", file_path, e))?;
    let total_size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let safe_off = if total_size > 16 {
        offset.min(total_size - 16)
    } else {
        0
    };
    let read_len = length.min(1024);

    f.seek(SeekFrom::Start(safe_off))
        .map_err(|e| format!("Seek error: {}", e))?;
    let mut buf = vec![0u8; read_len];
    let n = f.read(&mut buf).map_err(|e| format!("Read error: {}", e))?;
    buf.truncate(n);

    let mut lines = Vec::new();
    for (i, chunk) in buf.chunks(16).enumerate() {
        let addr = format!("{:06X}", safe_off + (i * 16) as u64);
        let hex = chunk
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");
        let ascii: String = chunk
            .iter()
            .map(|&b| if (32..=126).contains(&b) { b as char } else { '.' })
            .collect();
        lines.push(HexLine { addr, hex, ascii });
    }

    let fname = std::path::Path::new(file_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(file_path)
        .to_string();

    Ok(HexDumpReport {
        file: fname,
        arch: arch.to_string(),
        offset: safe_off,
        total_size,
        lines,
        engine: "rust-arm64-v11.0".to_string(),
    })
}

const MIPS_REGS: [&str; 32] = [
    "$zero", "$at", "$v0", "$v1", "$a0", "$a1", "$a2", "$a3",
    "$t0", "$t1", "$t2", "$t3", "$t4", "$t5", "$t6", "$t7",
    "$s0", "$s1", "$s2", "$s3", "$s4", "$s5", "$s6", "$s7",
    "$t8", "$t9", "$k0", "$k1", "$gp", "$sp", "$fp", "$ra",
];

fn decode_mips(word: u32, pc: u64) -> (String, String) {
    if word == 0 {
        return ("nop".to_string(), "".to_string());
    }
    let op = (word >> 26) & 0x3F;
    let rs = ((word >> 21) & 0x1F) as usize;
    let rt = ((word >> 16) & 0x1F) as usize;
    let rd = ((word >> 11) & 0x1F) as usize;
    let shamt = (word >> 6) & 0x1F;
    let funct = word & 0x3F;
    let imm = (word & 0xFFFF) as u16;
    let simm = imm as i16;
    let target = (word & 0x03FF_FFFF) << 2;

    match op {
        0x00 => match funct {
            0x00 => ("sll".into(), format!("{}, {}, {}", MIPS_REGS[rd], MIPS_REGS[rt], shamt)),
            0x02 => ("srl".into(), format!("{}, {}, {}", MIPS_REGS[rd], MIPS_REGS[rt], shamt)),
            0x08 => ("jr".into(), MIPS_REGS[rs].to_string()),
            0x09 => ("jalr".into(), format!("{}, {}", MIPS_REGS[rd], MIPS_REGS[rs])),
            0x0C => ("syscall".into(), "".into()),
            0x20 | 0x21 => ("addu".into(), format!("{}, {}, {}", MIPS_REGS[rd], MIPS_REGS[rs], MIPS_REGS[rt])),
            0x22 | 0x23 => ("subu".into(), format!("{}, {}, {}", MIPS_REGS[rd], MIPS_REGS[rs], MIPS_REGS[rt])),
            0x24 => ("and".into(), format!("{}, {}, {}", MIPS_REGS[rd], MIPS_REGS[rs], MIPS_REGS[rt])),
            0x25 => ("or".into(), format!("{}, {}, {}", MIPS_REGS[rd], MIPS_REGS[rs], MIPS_REGS[rt])),
            0x26 => ("xor".into(), format!("{}, {}, {}", MIPS_REGS[rd], MIPS_REGS[rs], MIPS_REGS[rt])),
            0x2A | 0x2B => ("sltu".into(), format!("{}, {}, {}", MIPS_REGS[rd], MIPS_REGS[rs], MIPS_REGS[rt])),
            _ => ("special".into(), format!("funct=0x{:02X}", funct)),
        },
        0x02 => ("j".into(), format!("0x{:08X}", ((pc as u32) & 0xF000_0000) | target)),
        0x03 => ("jal".into(), format!("0x{:08X}", ((pc as u32) & 0xF000_0000) | target)),
        0x04 => ("beq".into(), format!("{}, {}, 0x{:08X}", MIPS_REGS[rs], MIPS_REGS[rt], (pc as i64 + 4 + ((simm as i64) << 2)) as u32)),
        0x05 => ("bne".into(), format!("{}, {}, 0x{:08X}", MIPS_REGS[rs], MIPS_REGS[rt], (pc as i64 + 4 + ((simm as i64) << 2)) as u32)),
        0x08 | 0x09 => ("addiu".into(), format!("{}, {}, {}", MIPS_REGS[rt], MIPS_REGS[rs], simm)),
        0x0C => ("andi".into(), format!("{}, {}, 0x{:04X}", MIPS_REGS[rt], MIPS_REGS[rs], imm)),
        0x0D => ("ori".into(), format!("{}, {}, 0x{:04X}", MIPS_REGS[rt], MIPS_REGS[rs], imm)),
        0x0E => ("xori".into(), format!("{}, {}, 0x{:04X}", MIPS_REGS[rt], MIPS_REGS[rs], imm)),
        0x0F => ("lui".into(), format!("{}, 0x{:04X}", MIPS_REGS[rt], imm)),
        0x20 => ("lb".into(), format!("{}, {}({})", MIPS_REGS[rt], simm, MIPS_REGS[rs])),
        0x21 => ("lh".into(), format!("{}, {}({})", MIPS_REGS[rt], simm, MIPS_REGS[rs])),
        0x23 => ("lw".into(), format!("{}, {}({})", MIPS_REGS[rt], simm, MIPS_REGS[rs])),
        0x24 => ("lbu".into(), format!("{}, {}({})", MIPS_REGS[rt], simm, MIPS_REGS[rs])),
        0x25 => ("lhu".into(), format!("{}, {}({})", MIPS_REGS[rt], simm, MIPS_REGS[rs])),
        0x28 => ("sb".into(), format!("{}, {}({})", MIPS_REGS[rt], simm, MIPS_REGS[rs])),
        0x29 => ("sh".into(), format!("{}, {}({})", MIPS_REGS[rt], simm, MIPS_REGS[rs])),
        0x2B => ("sw".into(), format!("{}, {}({})", MIPS_REGS[rt], simm, MIPS_REGS[rs])),
        _ => (".word".into(), format!("0x{:08X}", word)),
    }
}

fn decode_m68k(code: &[u8], pc: u64) -> (usize, String, String) {
    if code.len() < 2 {
        return (1, "dc.b".into(), format!("0x{:02X}", code[0]));
    }
    let w = u16::from_be_bytes([code[0], code[1]]);
    match w {
        0x4E71 => (2, "nop".into(), "".into()),
        0x4E75 => (2, "rts".into(), "".into()),
        0x4E73 => (2, "rte".into(), "".into()),
        0x4EB9 if code.len() >= 6 => {
            let dst = u32::from_be_bytes([code[2], code[3], code[4], code[5]]);
            (6, "jsr".into(), format!("0x{:08X}", dst))
        }
        0x4EF9 if code.len() >= 6 => {
            let dst = u32::from_be_bytes([code[2], code[3], code[4], code[5]]);
            (6, "jmp".into(), format!("0x{:08X}", dst))
        }
        _ if (w & 0xF100) == 0x7000 => {
            let reg = (w >> 9) & 7;
            let imm = (w & 0xFF) as i8;
            (2, "moveq".into(), format!("#{}, d{}", imm, reg))
        }
        _ if (w & 0xFF00) == 0x6000 => {
            let disp = (w & 0xFF) as i8;
            if disp == 0 && code.len() >= 4 {
                let d16 = i16::from_be_bytes([code[2], code[3]]);
                (4, "bra.w".into(), format!("0x{:08X}", (pc as i64 + 2 + d16 as i64) as u32))
            } else {
                (2, "bra.s".into(), format!("0x{:08X}", (pc as i64 + 2 + disp as i64) as u32))
            }
        }
        _ if (w & 0xFF00) == 0x6100 => {
            let disp = (w & 0xFF) as i8;
            if disp == 0 && code.len() >= 4 {
                let d16 = i16::from_be_bytes([code[2], code[3]]);
                (4, "bsr.w".into(), format!("0x{:08X}", (pc as i64 + 2 + d16 as i64) as u32))
            } else {
                (2, "bsr.s".into(), format!("0x{:08X}", (pc as i64 + 2 + disp as i64) as u32))
            }
        }
        _ if (w & 0xFF00) == 0x6700 => {
            let disp = (w & 0xFF) as i8;
            (2, "beq.s".into(), format!("0x{:08X}", (pc as i64 + 2 + disp as i64) as u32))
        }
        _ if (w & 0xFF00) == 0x6600 => {
            let disp = (w & 0xFF) as i8;
            (2, "bne.s".into(), format!("0x{:08X}", (pc as i64 + 2 + disp as i64) as u32))
        }
        _ if (w & 0xF1C0) == 0x41C0 && code.len() >= 6 => {
            let reg = (w >> 9) & 7;
            let addr = u32::from_be_bytes([code[2], code[3], code[4], code[5]]);
            (6, "lea".into(), format!("0x{:08X}, a{}", addr, reg))
        }
        _ => (2, "dc.w".into(), format!("0x{:04X}", w)),
    }
}

fn decode_sh2(w: u16, pc: u64) -> (String, String) {
    let nib0 = (w >> 12) & 0xF;
    let rn = (w >> 8) & 0xF;
    let rm = (w >> 4) & 0xF;
    let imm8 = (w & 0xFF) as i8;

    match w {
        0x0009 => ("nop".into(), "".into()),
        0x000B => ("rts".into(), "".into()),
        0x002B => ("rte".into(), "".into()),
        _ => match nib0 {
            0xE => ("mov".into(), format!("#{}, r{}", imm8, rn)),
            0x7 => ("add".into(), format!("#{}, r{}", imm8, rn)),
            0x6 if (w & 0xF) == 3 => ("mov".into(), format!("r{}, r{}", rm, rn)),
            0x6 if (w & 0xF) == 2 => ("mov.l".into(), format!("@r{}, r{}", rm, rn)),
            0x2 if (w & 0xF) == 2 => ("mov.l".into(), format!("r{}, @r{}", rm, rn)),
            0x3 if (w & 0xF) == 0xC => ("add".into(), format!("r{}, r{}", rm, rn)),
            0xA => {
                let d12 = ((w & 0xFFF) as i16) << 4 >> 4;
                ("bra".into(), format!("0x{:08X}", (pc as i64 + 4 + (d12 as i64) * 2) as u32))
            }
            0xB => {
                let d12 = ((w & 0xFFF) as i16) << 4 >> 4;
                ("bsr".into(), format!("0x{:08X}", (pc as i64 + 4 + (d12 as i64) * 2) as u32))
            }
            _ => (".word".into(), format!("0x{:04X}", w)),
        },
    }
}

fn decode_8bit(code: &[u8], pc: u64, is_huc6280: bool) -> (usize, String, String) {
    let b0 = code[0];
    let b1 = code.get(1).copied().unwrap_or(0);
    let b2 = code.get(2).copied().unwrap_or(0);
    let w16 = u16::from_le_bytes([b1, b2]);

    if is_huc6280 {
        match b0 {
            0xEA => (1, "nop".into(), "".into()),
            0x60 => (1, "rts".into(), "".into()),
            0x40 => (1, "rti".into(), "".into()),
            0x78 => (1, "sei".into(), "".into()),
            0x58 => (1, "cli".into(), "".into()),
            0xD8 => (1, "cld".into(), "".into()),
            0xA9 if code.len() >= 2 => (2, "lda".into(), format!("#$ {:02X}", b1)),
            0xA2 if code.len() >= 2 => (2, "ldx".into(), format!("#$ {:02X}", b1)),
            0xA0 if code.len() >= 2 => (2, "ldy".into(), format!("#$ {:02X}", b1)),
            0x85 if code.len() >= 2 => (2, "sta".into(), format!("${:02X}", b1)),
            0x8D if code.len() >= 3 => (3, "sta".into(), format!("${:04X}", w16)),
            0xAD if code.len() >= 3 => (3, "lda".into(), format!("${:04X}", w16)),
            0x20 if code.len() >= 3 => (3, "jsr".into(), format!("${:04X}", w16)),
            0x4C if code.len() >= 3 => (3, "jmp".into(), format!("${:04X}", w16)),
            0x53 if code.len() >= 2 => (2, "tam".into(), format!("#${:02X}", b1)),
            0x43 if code.len() >= 2 => (2, "tma".into(), format!("#${:02X}", b1)),
            0xD0 if code.len() >= 2 => (2, "bne".into(), format!("${:04X}", (pc as i64 + 2 + (b1 as i8) as i64) as u16)),
            0xF0 if code.len() >= 2 => (2, "beq".into(), format!("${:04X}", (pc as i64 + 2 + (b1 as i8) as i64) as u16)),
            0x80 if code.len() >= 2 => (2, "bra".into(), format!("${:04X}", (pc as i64 + 2 + (b1 as i8) as i64) as u16)),
            _ => (1, ".db".into(), format!("${:02X}", b0)),
        }
    } else {
        // Z80 (MSX2 / PC-8801)
        match b0 {
            0x00 => (1, "nop".into(), "".into()),
            0xC9 => (1, "ret".into(), "".into()),
            0xF3 => (1, "di".into(), "".into()),
            0xFB => (1, "ei".into(), "".into()),
            0x76 => (1, "halt".into(), "".into()),
            0xAF => (1, "xor".into(), "a".into()),
            0x3E if code.len() >= 2 => (2, "ld".into(), format!("a, 0x{:02X}", b1)),
            0x06 if code.len() >= 2 => (2, "ld".into(), format!("b, 0x{:02X}", b1)),
            0x0E if code.len() >= 2 => (2, "ld".into(), format!("c, 0x{:02X}", b1)),
            0x21 if code.len() >= 3 => (3, "ld".into(), format!("hl, 0x{:04X}", w16)),
            0x11 if code.len() >= 3 => (3, "ld".into(), format!("de, 0x{:04X}", w16)),
            0x01 if code.len() >= 3 => (3, "ld".into(), format!("bc, 0x{:04X}", w16)),
            0xC3 if code.len() >= 3 => (3, "jp".into(), format!("0x{:04X}", w16)),
            0xCD if code.len() >= 3 => (3, "call".into(), format!("0x{:04X}", w16)),
            0xD3 if code.len() >= 2 => (2, "out".into(), format!("(0x{:02X}), a", b1)),
            0xDB if code.len() >= 2 => (2, "in".into(), format!("a, (0x{:02X})", b1)),
            0x18 if code.len() >= 2 => (2, "jr".into(), format!("0x{:04X}", (pc as i64 + 2 + (b1 as i8) as i64) as u16)),
            0x20 if code.len() >= 2 => (2, "jr".into(), format!("nz, 0x{:04X}", (pc as i64 + 2 + (b1 as i8) as i64) as u16)),
            0x28 if code.len() >= 2 => (2, "jr".into(), format!("z, 0x{:04X}", (pc as i64 + 2 + (b1 as i8) as i64) as u16)),
            _ => (1, "db".into(), format!("0x{:02X}", b0)),
        }
    }
}

pub fn disassemble_file(
    plat_key: &str,
    platform_name: &str,
    arch: &str,
    file_path: &str,
    offset: u64,
    count: usize,
) -> Result<DisasmReport, String> {
    let mut f = File::open(file_path).map_err(|e| format!("Failed to open {}: {}", file_path, e))?;
    let total_size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let safe_off = if total_size > 64 {
        offset.min(total_size - 64)
    } else {
        0
    };

    f.seek(SeekFrom::Start(safe_off))
        .map_err(|e| format!("Seek error: {}", e))?;
    let mut buf = vec![0u8; (count * 6).min(512)];
    let n = f.read(&mut buf).map_err(|e| format!("Read error: {}", e))?;
    buf.truncate(n);

    let mut instructions = Vec::with_capacity(count);
    let mut pos = 0usize;
    let key = plat_key.to_lowercase();

    while pos < buf.len() && instructions.len() < count {
        let cur_pc = safe_off + pos as u64;
        if key == "ps1" {
            if pos + 4 > buf.len() {
                break;
            }
            let w = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
            let (m, op) = decode_mips(w, cur_pc);
            let bytes = format!("{:02X} {:02X} {:02X} {:02X}", buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]);
            instructions.push(DisasmInsn {
                addr: format!("0x{:08X}", cur_pc),
                bytes,
                mnemonic: m,
                op_str: op,
            });
            pos += 4;
        } else if key == "segacd" {
            let (step, m, op) = decode_m68k(&buf[pos..], cur_pc);
            let slice = &buf[pos..(pos + step).min(buf.len())];
            let bytes = slice.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
            instructions.push(DisasmInsn {
                addr: format!("0x{:08X}", cur_pc),
                bytes,
                mnemonic: m,
                op_str: op,
            });
            pos += step;
        } else if key == "saturn" {
            if pos + 2 > buf.len() {
                break;
            }
            let w = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
            let (m, op) = decode_sh2(w, cur_pc);
            let bytes = format!("{:02X} {:02X}", buf[pos], buf[pos + 1]);
            instructions.push(DisasmInsn {
                addr: format!("0x{:08X}", cur_pc),
                bytes,
                mnemonic: m,
                op_str: op,
            });
            pos += 2;
        } else {
            let is_huc = key == "pce";
            let (step, m, op) = decode_8bit(&buf[pos..], cur_pc, is_huc);
            let slice = &buf[pos..(pos + step).min(buf.len())];
            let bytes = slice.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
            instructions.push(DisasmInsn {
                addr: format!("0x{:08X}", cur_pc),
                bytes,
                mnemonic: m,
                op_str: op,
            });
            pos += step;
        }
    }

    Ok(DisasmReport {
        platform: platform_name.to_string(),
        arch: arch.to_string(),
        file: file_path.to_string(),
        offset: safe_off,
        instructions,
        engine: "rust-arm64-v11.0-native-disasm".to_string(),
    })
}
