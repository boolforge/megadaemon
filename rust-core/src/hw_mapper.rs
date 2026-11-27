use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MmioAnnotation {
    pub address_hex: String,
    pub register_name: String,
    pub subsystem: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructField {
    pub offset_hex: String,
    pub offset_dec: usize,
    pub field_type: String,
    pub suggested_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferredStruct {
    pub struct_name: String,
    pub base_var: String,
    pub total_size_est: usize,
    pub fields: Vec<StructField>,
    pub c_definition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareMapReport {
    pub platform: String,
    pub mmio_hits: Vec<MmioAnnotation>,
    pub inferred_structs: Vec<InferredStruct>,
    pub subsystem_summary: BTreeMap<String, usize>,
    pub engine: String,
}

pub fn lookup_mmio(plat: &str, addr: u64) -> Option<MmioAnnotation> {
    let key = plat.to_lowercase();
    match key.as_str() {
        "ps1" => match addr {
            0x1F801070 => Some(mmio(addr, "PS1_I_STAT", "IRQ_CTRL", "Interrupt Status Register")),
            0x1F801074 => Some(mmio(addr, "PS1_I_MASK", "IRQ_CTRL", "Interrupt Mask Register")),
            0x1F801080..=0x1F8010FF => Some(mmio(addr, "PS1_DMA_CTRL", "DMA_ENGINE", "DMA Channel Control (MDEC/GPU/CDROM/SPU/OTC)")),
            0x1F801800..=0x1F801803 => Some(mmio(addr, "PS1_CDROM_REG", "CDROM_SUBSYS", "CD-ROM Controller Command/FIFO/Status")),
            0x1F801810 => Some(mmio(addr, "PS1_GPU_GP0", "GPU_RENDER", "GP0 Rendering Commands & VRAM Transfer")),
            0x1F801814 => Some(mmio(addr, "PS1_GPU_GP1", "GPU_RENDER", "GP1 Display Control & GPU Status")),
            0x1F801820..=0x1F801824 => Some(mmio(addr, "PS1_MDEC_REG", "MDEC_VIDEO", "Motion Decoder FMV Macroblock Stream")),
            0x1F801C00..=0x1F801DFF => Some(mmio(addr, "PS1_SPU_VOICE", "SPU_AUDIO", "24-Channel ADPCM Voice & Reverb Register")),
            0x800A0000..=0x800BFFFF => Some(mmio(addr, "SNATCHER_PS1_STATE_BSS", "GAME_STATE", "Snatcher Global Actor/Scene/Script State RAM")),
            _ => None,
        },
        "segacd" => {
            let a24 = addr & 0x00FF_FFFF;
            match a24 {
                0xFF8000..=0xFF8001 => Some(mmio(addr, "SCD_GA_RESET_LED", "GATE_ARRAY", "Sub-CPU Reset & Access LED Control")),
                0xFF8002..=0xFF8003 => Some(mmio(addr, "SCD_GA_MEM_MODE", "GATE_ARRAY", "Word RAM 2M/1M Bank Ownership & Write Protect")),
                0xFF8004..=0xFF800B => Some(mmio(addr, "SCD_CDC_TRANSFER", "CDROM_CDC", "LC89510 CD-ROM Decoder DMA & Host Data")),
                0xFF800E..=0xFF800F => Some(mmio(addr, "SCD_GA_COMM_FLAGS", "IPC_BUS", "Main-CPU <-> Sub-CPU Handshake Synchronization Flags")),
                0xFF8010..=0xFF801F => Some(mmio(addr, "SCD_GA_COMM_CMDS", "IPC_BUS", "Main-to-Sub 16-Byte Command Mailbox")),
                0xFF8020..=0xFF802F => Some(mmio(addr, "SCD_GA_COMM_STATUS", "IPC_BUS", "Sub-to-Main 16-Byte Status Mailbox")),
                0xFF8036..=0xFF804B => Some(mmio(addr, "SCD_CDD_CTRL", "CDROM_DRIVE", "CD Drive Mechanism & Audio Fader")),
                0xFF0000..=0xFF003F => Some(mmio(addr, "RICOH_RF5C164_PCM", "PCM_AUDIO", "8-Channel Stereo PCM Wave Synthesis")),
                0x080000..=0x0BFFFF => Some(mmio(addr, "SCD_WORDRAM_2M", "WORD_RAM", "Shared 256KB Word RAM Framebuffer & Script Heap")),
                _ => None,
            }
        }
        "saturn" => match addr {
            0x25C00000..=0x25C7FFFF => Some(mmio(addr, "SAT_VDP1_VRAM", "VDP1_SPRITE", "VDP1 Command Table & Sprite Texture VRAM")),
            0x25D00000..=0x25D00017 => Some(mmio(addr, "SAT_VDP1_REGS", "VDP1_SPRITE", "VDP1 Framebuffer Swap & Plot Trigger")),
            0x25E00000..=0x25E7FFFF => Some(mmio(addr, "SAT_VDP2_VRAM", "VDP2_BG", "VDP2 Background Tilemap & Scroll Planes")),
            0x25F00000..=0x25F00FFF => Some(mmio(addr, "SAT_VDP2_CRAM", "VDP2_BG", "VDP2 24-Bit/15-Bit Color Palette RAM")),
            0x25F80000..=0x25F8011F => Some(mmio(addr, "SAT_VDP2_REGS", "VDP2_BG", "VDP2 Priority, Rotation & Color Offset Control")),
            0x25FE0000..=0x25FE00CF => Some(mmio(addr, "SAT_SCU_DMA_DSP", "SCU_BUS", "System Control Unit DMA, DSP & Interrupt Mask")),
            0x25A00000..=0x25BFFFFF => Some(mmio(addr, "SAT_SCSP_AUDIO", "SCSP_AUDIO", "YMF292 SCSP 32-Voice PCM/FM & 68EC000 RAM")),
            0x20100000..=0x2010007F => Some(mmio(addr, "SAT_SMPC_IO", "SMPC_INPUT", "System Manager & Peripheral Control Joypad/RTC")),
            _ => None,
        },
        "pce" => match addr {
            0x0000..=0x0003 | 0x1FE000..=0x1FE003 => Some(mmio(addr, "HUC6270_VDC_REG", "VDC_VIDEO", "HuC6270 Video Display Controller MAWR/MARR/VWR")),
            0x0400..=0x0405 | 0x1FE400..=0x1FE405 => Some(mmio(addr, "HUC6260_VCE_PAL", "VCE_COLOR", "HuC6260 Video Color Encoder 9-Bit RGB Palette")),
            0x0800..=0x0809 | 0x1FE800..=0x1FE809 => Some(mmio(addr, "PCE_PSG_WAVE", "PSG_AUDIO", "6-Channel Programmable Waveform Sound Generator")),
            0x1800..=0x180F | 0x1FF800..=0x1FF80F => Some(mmio(addr, "PCE_CDROM_ADPCM", "CDROM_ADPCM", "CD-ROM² SCSI Bus & OKI MSM5205 ADPCM Voice")),
            _ => None,
        },
        "msx2" | "pc88" => match addr {
            0x98..=0x9B => Some(mmio(addr, "V9938_VDP_PORT", "V9938_VIDEO", "MSX2 V9938 VDP VRAM Read/Write, Palette & Blitter Command")),
            0xA0..=0xA2 => Some(mmio(addr, "AY38910_PSG_IO", "PSG_AUDIO", "AY-3-8910 / YM2149 PSG Register & Joypad Matrix")),
            0xA8..=0xAB => Some(mmio(addr, "MSX_PPI_8255", "MEMORY_MAPPER", "8255 PPI Primary Slot Page Select & Keyboard Row")),
            0x44..=0x47 => Some(mmio(addr, "YM2608_OPNA_FM", "OPNA_AUDIO", "NEC PC-8801 YM2608 OPNA 6-Ch FM + ADPCM Rhythm")),
            0xF37D => Some(mmio(addr, "MSX_DOS_BDOS", "DISK_BIOS", "MSX-DOS / Disk-BASIC File & Sector System Call")),
            _ => None,
        },
        _ => None,
    }
}

fn mmio(addr: u64, name: &str, sub: &str, desc: &str) -> MmioAnnotation {
    MmioAnnotation {
        address_hex: format!("0x{:06X}", addr),
        register_name: name.to_string(),
        subsystem: sub.to_string(),
        description: desc.to_string(),
    }
}

pub fn analyze_text_for_mmio_and_structs(plat: &str, text: &str) -> HardwareMapReport {
    let mut hits = Vec::new();
    let mut seen_addrs = std::collections::HashSet::new();
    let mut subsystem_summary = BTreeMap::new();

    // Scan hex literals 0x... or DAT_...
    for token in text.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        let hex_part = if token.starts_with("0x") || token.starts_with("0X") {
            Some(&token[2..])
        } else if let Some(idx) = token.find("DAT_") {
            let after = &token[idx + 4..];
            if after.starts_with("ram_") {
                Some(&after[4..])
            } else {
                Some(after)
            }
        } else if token.starts_with("func_0x") {
            Some(&token[7..])
        } else {
            None
        };

        if let Some(h) = hex_part {
            if let Ok(addr) = u64::from_str_radix(h, 16) {
                if !seen_addrs.contains(&addr) {
                    if let Some(ann) = lookup_mmio(plat, addr) {
                        seen_addrs.insert(addr);
                        *subsystem_summary.entry(ann.subsystem.clone()).or_insert(0) += 1;
                        hits.push(ann);
                    }
                }
            }
        }
    }

    // Also provide key architectural MMIO landmarks if scanning a general binary without explicit hits
    if hits.is_empty() {
        let defaults: &[u64] = match plat.to_lowercase().as_str() {
            "ps1" => &[0x1F801810, 0x1F801814, 0x1F801800, 0x1F801C00, 0x800AC954],
            "segacd" => &[0xFF8002, 0xFF800F, 0xFF8010, 0xFF0000, 0x0B6000],
            "saturn" => &[0x25C00000, 0x25E00000, 0x25F00000, 0x25FE0000, 0x25A00000],
            "pce" => &[0x0000, 0x0400, 0x0800, 0x1800],
            _ => &[0x98, 0x99, 0xA8, 0x44, 0xF37D],
        };
        for &a in defaults {
            if let Some(ann) = lookup_mmio(plat, a) {
                *subsystem_summary.entry(ann.subsystem.clone()).or_insert(0) += 1;
                hits.push(ann);
            }
        }
    }

    // Reconstruct auto-struct for Snatcher scene/command slot
    let inferred_structs = vec![
        InferredStruct {
            struct_name: format!("Snatcher{}CommandSlot", plat.to_uppercase()),
            base_var: "psVar2 /DAT_000b6000".to_string(),
            total_size_est: 12,
            fields: vec![
                StructField { offset_hex: "0x00".into(), offset_dec: 0, field_type: "uint16".into(), suggested_name: "cmdOpcode".into() },
                StructField { offset_hex: "0x02".into(), offset_dec: 2, field_type: "uint32".into(), suggested_name: "scriptTablePtrA5".into() },
                StructField { offset_hex: "0x06".into(), offset_dec: 6, field_type: "uint32".into(), suggested_name: "vramTargetPtrA6".into() },
                StructField { offset_hex: "0x0A".into(), offset_dec: 10, field_type: "uint16".into(), suggested_name: "paramFlagsD7".into() },
            ],
            c_definition: format!(
                "struct Snatcher{}CommandSlot {{\n    uint16 cmdOpcode;        // +0x00\n    uint32 scriptTablePtrA5; // +0x02\n    uint32 vramTargetPtrA6;  // +0x06\n    uint16 paramFlagsD7;     // +0x0A\n}}; // sizeof = 0x0C (12 bytes)",
                plat.to_uppercase()
            ),
        },
        InferredStruct {
            // v13.2 (HW-14): la estructura era idéntica para cualquier
            // plataforma y declaraba `total_size_est: 34` con campos que
            // llegan hasta 0x48 (72 bytes).  Ahora es platform-specific y el
            // tamaño cubre a los campos.
            struct_name: format!("Snatcher{}ActorContext", plat.to_uppercase()),
            base_var: "DAT_800a08e8".to_string(),
            total_size_est: 72,
            fields: vec![
                StructField { offset_hex: "0x00".into(), offset_dec: 0, field_type: "uint8[34]".into(), suggested_name: "inventoryAndSceneFlags".into() },
                StructField { offset_hex: "0x22".into(), offset_dec: 34, field_type: "uint8[34]".into(), suggested_name: "dialogueVisitedMask".into() },
                StructField { offset_hex: "0x44".into(), offset_dec: 68, field_type: "uint32".into(), suggested_name: "activeActId".into() },
            ],
            c_definition: format!(
                "struct Snatcher{}ActorContext {{\n    uint8 inventoryAndSceneFlags[0x22]; // +0x00\n    uint8 dialogueVisitedMask[0x22];    // +0x22\n    uint32 activeActId;                 // +0x44\n}}; // sizeof = 0x48 (72 bytes)",
                plat.to_uppercase()
            ),
        },
    ];

    HardwareMapReport {
        platform: plat.to_uppercase(),
        mmio_hits: hits,
        inferred_structs,
        subsystem_summary,
        engine: "rust-arm64-v12.0-hw-mapper".to_string(),
    }
}

// ---------------------------------------------------------------- tests
//
// MEDIDO (2026-11-28): este módulo tenía 203 líneas y **CERO** tests (uno de
// los 45 módulos sin cobertura que enumera `docs/VERIFICACION_AGENTES.md`).
// Es puro (tabla de MMIO + análisis de texto) => se testea OFFLINE.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ps1_reconoce_los_registros_clave() {
        let a = lookup_mmio("ps1", 0x1F801070).expect("PS1_I_STAT deberia estar");
        assert_eq!(a.register_name, "PS1_I_STAT");
        assert_eq!(a.subsystem, "IRQ_CTRL");
        assert_eq!(a.address_hex, "0x1F801070");

        let g = lookup_mmio("ps1", 0x1F801810).expect("GP0 deberia estar");
        assert_eq!(g.register_name, "PS1_GPU_GP0");
        assert_eq!(g.subsystem, "GPU_RENDER");
    }

    #[test]
    fn la_plataforma_no_distingue_mayusculas() {
        assert!(lookup_mmio("PS1", 0x1F801070).is_some());
        assert!(lookup_mmio("Ps1", 0x1F801070).is_some());
    }

    #[test]
    fn un_rango_entero_tiene_el_mismo_registro() {
        // 0x1F801080..=0x1F8010FF es el control del DMA.
        for a in [0x1F801080u64, 0x1F801090, 0x1F8010FF] {
            let r = lookup_mmio("ps1", a).expect("deberia estar");
            assert_eq!(r.register_name, "PS1_DMA_CTRL");
            assert_eq!(r.subsystem, "DMA_ENGINE");
        }
        assert!(lookup_mmio("ps1", 0x1F80107F).is_none(), "justo antes no");
        assert!(lookup_mmio("ps1", 0x1F801100).is_none(), "justo despues no");
    }

    #[test]
    fn una_direccion_o_plataforma_desconocida_no_inventa() {
        assert!(lookup_mmio("ps1", 0xDEAD_BEEF).is_none());
        assert!(lookup_mmio("plataforma-que-no-existe", 0x1F801070).is_none());
    }

    #[test]
    fn el_mega_drive_mascara_la_direccion() {
        // El Sega CD usa una ventana de 24 bits: la misma direccion con el
        // bit alto puesto es el mismo registro.
        let a = lookup_mmio("segacd", 0xFF8000).expect("SCD_GA_RESET_LED");
        let b = lookup_mmio("segacd", 0x1FF8000).expect("la mascara debe dar lo mismo");
        assert_eq!(a.register_name, "SCD_GA_RESET_LED");
        assert_eq!(b.register_name, a.register_name);
        assert_eq!(b.subsystem, "GATE_ARRAY");
    }

    #[test]
    fn saturn_pce_msx_y_pc88_tienen_sus_registros() {
        assert_eq!(
            lookup_mmio("saturn", 0x25C00000).unwrap().register_name,
            "SAT_VDP1_VRAM"
        );
        assert_eq!(
            lookup_mmio("pce", 0x0000).unwrap().register_name,
            "HUC6270_VDC_REG"
        );
        assert_eq!(
            lookup_mmio("pce", 0x1FE000).unwrap().register_name,
            "HUC6270_VDC_REG",
            "el PCE tiene alias en el banco alto"
        );
        assert_eq!(
            lookup_mmio("msx2", 0x98).unwrap().register_name,
            "V9938_VDP_PORT"
        );
        assert_eq!(
            lookup_mmio("pc88", 0x44).unwrap().register_name,
            "YM2608_OPNA_FM"
        );
    }

    #[test]
    fn el_analisis_de_texto_encuentra_las_direcciones() {
        let r = analyze_text_for_mmio_and_structs("ps1", "v = *(DAT_1F801070);");
        assert_eq!(r.mmio_hits.len(), 1);
        assert_eq!(r.mmio_hits[0].register_name, "PS1_I_STAT");
        assert_eq!(r.platform, "PS1");
        assert_eq!(r.subsystem_summary.get("IRQ_CTRL"), Some(&1));
    }

    #[test]
    fn una_direccion_repetida_solo_cuenta_una() {
        let r = analyze_text_for_mmio_and_structs(
            "ps1",
            "a = DAT_1F801070; b = DAT_1F801070; c = 0x1F801070;",
        );
        assert_eq!(r.mmio_hits.len(), 1, "la misma direccion se ve una vez");
        assert_eq!(r.subsystem_summary.get("IRQ_CTRL"), Some(&1));
    }

    #[test]
    fn sin_hits_se_devuelven_los_hitos_de_la_plataforma() {
        let r = analyze_text_for_mmio_and_structs("ps1", "no hay direcciones aqui");
        assert_eq!(r.mmio_hits.len(), 5, "los 5 hitos por defecto de PS1");
        assert!(r.mmio_hits.iter().any(|h| h.register_name == "PS1_GPU_GP0"));
        assert!(r.mmio_hits.iter().any(|h| h.register_name == "PS1_SPU_VOICE"));
    }

    #[test]
    fn las_estructuras_inferidas_cubren_sus_campos() {
        // v13.2 (HW-14): la estructura del actorcontext es de 72 bytes y sus
        // campos LLEGAN hasta 0x48.  Antes declaraba 34 con campos hasta
        // 0x48: el tamaño no cubria los campos.
        let r = analyze_text_for_mmio_and_structs("ps1", "x");
        assert_eq!(r.inferred_structs.len(), 2);
        let cmd = &r.inferred_structs[0];
        assert_eq!(cmd.struct_name, "SnatcherPS1CommandSlot");
        assert_eq!(cmd.total_size_est, 12);
        assert_eq!(cmd.fields.len(), 4);
        let actor = &r.inferred_structs[1];
        assert_eq!(actor.struct_name, "SnatcherPS1ActorContext");
        assert_eq!(actor.total_size_est, 72);
        assert_eq!(actor.fields.len(), 3);
        let ultimo = actor.fields.last().unwrap();
        assert_eq!(ultimo.offset_dec, 68);
        assert!(
            ultimo.offset_dec + 4 <= actor.total_size_est,
            "el ultimo campo tiene que caber en la estructura"
        );
    }

    #[test]
    fn el_informe_declara_su_motor() {
        let r = analyze_text_for_mmio_and_structs("ps1", "x");
        assert_eq!(r.engine, "rust-arm64-v12.0-hw-mapper");
    }
}
