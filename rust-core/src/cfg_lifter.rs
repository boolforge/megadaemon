use crate::{disasm, hw_mapper};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgEdge {
    pub from_block: String,
    pub to_block: String,
    pub edge_type: String, // "fallthrough" | "branch_taken" | "unconditional" | "call"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    pub block_id: String,
    pub start_addr: String,
    pub end_addr: String,
    pub insn_count: usize,
    pub terminator: String,
    pub instructions: Vec<disasm::DisasmInsn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgReport {
    pub status: String,
    pub platform: String,
    pub arch: String,
    pub file: String,
    pub entry_offset: u64,
    pub total_instructions: usize,
    pub total_basic_blocks: usize,
    pub total_edges: usize,
    pub cyclomatic_complexity_mccabe: usize,
    pub call_targets: Vec<String>,
    pub basic_blocks: Vec<BasicBlock>,
    pub edges: Vec<CfgEdge>,
    pub mmio_annotations: Vec<hw_mapper::MmioAnnotation>,
    pub mermaid_graph: String,
    pub lifted_scummvm_cpp: String,
    pub engine: String,
}

fn is_branch_mnemonic(m: &str) -> bool {
    matches!(
        m,
        "beq" | "bne" | "beq.s" | "bne.s" | "bra" | "bra.s" | "bra.w" | "j" | "jp" | "jr" | "jmp"
    )
}

fn is_cond_branch(m: &str) -> bool {
    matches!(m, "beq" | "bne" | "beq.s" | "bne.s")
}

fn is_call_mnemonic(m: &str) -> bool {
    matches!(m, "jal" | "jalr" | "jsr" | "bsr" | "bsr.s" | "bsr.w" | "call")
}

fn is_return_mnemonic(m: &str) -> bool {
    matches!(m, "rts" | "rte" | "rti" | "ret" | "syscall")
}

pub fn lift_cfg(
    plat_key: &str,
    platform_name: &str,
    arch: &str,
    file_path: &str,
    offset: u64,
    max_insns: usize,
) -> Result<CfgReport, String> {
    let count = max_insns.clamp(8, 96);
    let dis = disasm::disassemble_file(plat_key, platform_name, arch, file_path, offset, count)?;
    let insns = &dis.instructions;
    if insns.is_empty() {
        return Err("No instructions decoded for CFG".into());
    }

    let mut leader_indices = BTreeSet::new();
    leader_indices.insert(0usize);
    let mut call_targets = Vec::new();

    for (i, ins) in insns.iter().enumerate() {
        let m = ins.mnemonic.as_str();
        let is_jr_ra = m == "jr" && ins.op_str.contains("$ra");
        if is_branch_mnemonic(m) || is_call_mnemonic(m) || is_return_mnemonic(m) || is_jr_ra {
            if i + 1 < insns.len() {
                leader_indices.insert(i + 1);
            }
            if is_call_mnemonic(m) && !ins.op_str.is_empty() && !call_targets.contains(&ins.op_str) {
                call_targets.push(ins.op_str.clone());
            }
        }
    }

    // Group into basic blocks (also split every 8 instructions if no branch occurred so CFG stays granular)
    let mut final_leaders = BTreeSet::new();
    let mut last_l = 0usize;
    for &l in &leader_indices {
        while l.saturating_sub(last_l) > 8 {
            last_l += 8;
            final_leaders.insert(last_l);
        }
        final_leaders.insert(l);
        last_l = l;
    }
    let leaders_vec: Vec<usize> = final_leaders.into_iter().collect();

    let mut basic_blocks = Vec::new();
    let mut edges = Vec::new();

    for (b_idx, &start_i) in leaders_vec.iter().enumerate() {
        let end_i = leaders_vec
            .get(b_idx + 1)
            .copied()
            .unwrap_or(insns.len())
            .min(insns.len());
        if start_i >= end_i {
            continue;
        }
        let slice = insns[start_i..end_i].to_vec();
        let first_addr = slice.first().map(|x| x.addr.clone()).unwrap_or_default();
        let last_ins = slice.last().unwrap();
        let last_addr = last_ins.addr.clone();
        let block_id = format!("BB_{:02}", b_idx);
        let term = format!("{} {}", last_ins.mnemonic, last_ins.op_str).trim().to_string();

        let m = last_ins.mnemonic.as_str();
        let is_jr_ra = m == "jr" && last_ins.op_str.contains("$ra");

        if !is_return_mnemonic(m) && !is_jr_ra {
            if b_idx + 1 < leaders_vec.len() {
                let next_id = format!("BB_{:02}", b_idx + 1);
                edges.push(CfgEdge {
                    from_block: block_id.clone(),
                    to_block: next_id.clone(),
                    edge_type: if is_cond_branch(m) {
                        "branch_false".into()
                    } else {
                        "fallthrough".into()
                    },
                });
            }
            if is_cond_branch(m) {
                let target_id = if b_idx + 2 < leaders_vec.len() {
                    format!("BB_{:02}", b_idx + 2)
                } else {
                    format!("BB_{:02}", b_idx)
                };
                edges.push(CfgEdge {
                    from_block: block_id.clone(),
                    to_block: target_id,
                    edge_type: "branch_taken".into(),
                });
            }
        }

        basic_blocks.push(BasicBlock {
            block_id,
            start_addr: first_addr,
            end_addr: last_addr,
            insn_count: slice.len(),
            terminator: term,
            instructions: slice,
        });
    }

    let n_nodes = basic_blocks.len().max(1);
    let n_edges = edges.len();
    let mccabe = if n_edges + 2 > n_nodes {
        (n_edges + 2) - n_nodes
    } else {
        1
    };

    // Build Mermaid diagram
    let mut mermaid = String::from("flowchart TD\n");
    for bb in &basic_blocks {
        let preview = bb
            .instructions
            .iter()
            .take(3)
            .map(|i| format!("{} {}", i.mnemonic, i.op_str))
            .collect::<Vec<_>>()
            .join(" | ");
        mermaid.push_str(&format!(
            "    {}[\"{} ({})\\n{}\"]\n",
            bb.block_id,
            bb.block_id,
            bb.start_addr,
            preview.replace('"', "'")
        ));
    }
    for e in &edges {
        mermaid.push_str(&format!(
            "    {} -->|\"{}\"| {}\n",
            e.from_block, e.edge_type, e.to_block
        ));
    }

    let all_text = insns
        .iter()
        .map(|i| format!("{} {} {}", i.addr, i.mnemonic, i.op_str))
        .collect::<Vec<_>>()
        .join("\n");
    let hw_rep = hw_mapper::analyze_text_for_mmio_and_structs(plat_key, &all_text);

    // Synthesize lifted ScummVM C++ code from Basic Blocks
    let mut cpp = format!(
        "// Auto-Lifted CFG Function -> ScummVM SnatcherEngine C++ ({} | {})\n\
         // Entry: 0x{:06X} | Basic Blocks: {} | McCabe Complexity: {}\n\
         void SnatcherEngine::execLiftedSubroutine_{}_{:06X}() {{\n",
        platform_name,
        arch,
        dis.offset,
        n_nodes,
        mccabe,
        plat_key.to_lowercase(),
        dis.offset
    );
    for bb in &basic_blocks {
        cpp.push_str(&format!("{}:\n    // Range: {} .. {}\n", bb.block_id, bb.start_addr, bb.end_addr));
        for ins in &bb.instructions {
            cpp.push_str(&format!("    // {} : {:12} {} {}\n", ins.addr, ins.bytes, ins.mnemonic, ins.op_str));
        }
        cpp.push_str(&format!("    stepVlaBusCycle(\"{}\", {});\n", bb.block_id, bb.insn_count));
    }
    cpp.push_str("}\n");

    Ok(CfgReport {
        status: "OK".to_string(),
        platform: platform_name.to_string(),
        arch: arch.to_string(),
        file: file_path.to_string(),
        entry_offset: dis.offset,
        total_instructions: insns.len(),
        total_basic_blocks: basic_blocks.len(),
        total_edges: edges.len(),
        cyclomatic_complexity_mccabe: mccabe,
        call_targets,
        basic_blocks,
        edges,
        mmio_annotations: hw_rep.mmio_hits,
        mermaid_graph: mermaid,
        lifted_scummvm_cpp: cpp,
        engine: "rust-arm64-v12.0-cfg-lifter".to_string(),
    })
}
