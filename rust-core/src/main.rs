mod bindiff;
mod cfg_lifter;
mod cockpit_server;
mod crypto_scan;
mod deobfuscate;
mod disasm;
mod entropy;
mod freezer;
mod hw_mapper;
mod npu_bridge;
mod pattern;
mod pointers;
mod rea_assistant;
mod rom_locator;
mod seere_v11;
mod shm;
mod snapshot;
mod unifier;

use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};

fn print_usage() {
    eprintln!("MegaDaemon Native Rust Core v12.0 Omega (ARMv8.2-A + Exynos 2100 NPU + REA Assistant)");
    eprintln!("Usage: megadaemon-core <command> [args...]");
    eprintln!("Commands:");
    eprintln!("  cockpit [port]                              Launch Native Multi-Threaded Rust Cockpit HTTP Server (default: 3000)");
    eprintln!("  unifier                                     Scan & Unify All 9 Projects Across the Device Ecosystem");
    eprintln!("  rea <plat> [query] [max]                    REA Ghidra 2.7MB C Corpus Indexer, Semantic Classifier & ScummVM C++ Transpiler");
    eprintln!("  cfg <plat> [offset] [count]                 Native Control Flow Graph (CFG) Lifter, McCabe Complexity & Mermaid Generator");
    eprintln!("  bindiff <plat>                              Multi-ISA Cross-Platform Function Matcher (Diaphora in Rust)");
    eprintln!("  hw-mmio <plat>                              Hardware MMIO Register Annotator & Auto-Struct Reconstructor");
    eprintln!("  crypto-scan <plat>                          Shift-JIS Japanese String Decoder & Konami LZSS/RLE Compression Scanner");
    eprintln!("  seere <query>                               Execute SEERE v12 GBNF KV-Pinned Harness (with D, F, X RE Opcodes)");
    eprintln!("  seere-selftest                              Run SEERE v12 Assertion Suites & Export Shared GBNF/Prompts");
    eprintln!("  npu-probe                                   Probe Exynos 2100 NPU (/dev/vertex10, EDEN, NNAPI, KleidiAI) & Pin Cores 4-7");
    eprintln!("  locate-roms [refresh]                       Native Multi-Platform ROM & Binary Auto-Discovery");
    eprintln!("  disasm <plat> <file> [offset] [count]       Native Multi-Arch Disassembler (MIPS, M68K, SH-2, Z80, HuC6280)");
    eprintln!("  hex <file> [offset] [length]                Native Hex & ASCII Inspector");
    eprintln!("  deobfuscate <plat> <file>                   Native Konami XOR Script Deobfuscator");
    eprintln!("  dumper <plat> <file>                        Native Asset Signature Extractor");
    eprintln!("  entropy <file> [block_size]                 Shannon Entropy Block Classifier");
    eprintln!("  pattern <file> <pattern_str> [max_matches]  Wildcard AOB Pattern Scanner");
    eprintln!("  pointers <file> <base_hex> [is_be] [max]    32-bit Pointer Table Miner");
    eprintln!("  diff <file_a> <file_b> [base_hex] [max]     Memory Snapshot Differential Engine");
    eprintln!("  shm-status                                  Zero-Copy VLA SHM Bus Header Status");
    eprintln!("  shm-frame-bmp                               Render Live 24-bit BMP Frame from SHM");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "cockpit" => {
            let port = args.get(2).and_then(|s| s.parse::<u16>().ok()).unwrap_or(3000);
            if let Err(e) = cockpit_server::run_server(port) {
                eprintln!("Cockpit server error: {}", e);
                std::process::exit(1);
            }
        }

        "unifier" => {
            let rep = unifier::scan_unified_ecosystem();
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "rea" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            let query = args.get(3).map(|s| s.as_str()).unwrap_or("");
            let max_r = args.get(4).and_then(|s| s.parse::<usize>().ok()).unwrap_or(5);
            let rep = rea_assistant::analyze_ghidra_corpus(plat, query, max_r);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "cfg" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            let offset = args.get(3).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            let count = args.get(4).and_then(|s| s.parse::<usize>().ok()).unwrap_or(28);
            if let Some(rom) = rom_locator::resolve_binary(plat) {
                match cfg_lifter::lift_cfg(plat, &rom.platform_name, &rom.arch, &rom.path, offset, count) {
                    Ok(rep) => println!("{}", serde_json::to_string_pretty(&rep).unwrap()),
                    Err(e) => {
                        eprintln!("CFG error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Binary for platform '{}' not found", plat);
                std::process::exit(1);
            }
        }

        "bindiff" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            let rep = bindiff::run_cross_platform_bindiff(plat);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "hw-mmio" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            let (_, _, _, c_path, _) = rom_locator::platform_metadata(&plat.to_lowercase());
            let sample = fs::read_to_string(c_path)
                .map(|s| s[..s.len().min(65536)].to_string())
                .unwrap_or_default();
            let rep = hw_mapper::analyze_text_for_mmio_and_structs(plat, &sample);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "crypto-scan" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            if let Some(rom) = rom_locator::resolve_binary(plat) {
                let rep = crypto_scan::scan_crypto_and_encodings(plat, &rom.path);
                println!("{}", serde_json::to_string_pretty(&rep).unwrap());
            } else {
                eprintln!("Binary for platform '{}' not found", plat);
                std::process::exit(1);
            }
        }

        "seere" => {
            if args.len() < 3 {
                eprintln!("Usage: megadaemon-core seere <query>");
                std::process::exit(1);
            }
            let query = args[2..].join(" ");
            let res = seere_v11::run_seere_v11(&query, 4, true, false, false, true, true);
            println!("{}", serde_json::to_string(&res).unwrap());
        }

        "seere-selftest" => match seere_v11::selftest() {
            Ok(msg) => println!("{}", msg),
            Err(e) => {
                eprintln!("SEERE v12 selftest failed: {}", e);
                std::process::exit(1);
            }
        },

        "npu-probe" => {
            let rep = npu_bridge::probe_exynos_npu();
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "locate-roms" => {
            let refresh = args.get(2).map(|s| s == "1" || s == "true").unwrap_or(false);
            let roms = rom_locator::locate_all(refresh);
            println!("{}", serde_json::to_string(&roms).unwrap());
        }

        "disasm" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core disasm <plat> <file> [offset] [count]");
                std::process::exit(1);
            }
            let plat = &args[2];
            let file_path = &args[3];
            let offset = args.get(4).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            let count = args.get(5).and_then(|s| s.parse::<usize>().ok()).unwrap_or(16);
            let (plat_name, arch, _, _, _) = rom_locator::platform_metadata(plat);
            match disasm::disassemble_file(plat, plat_name, arch, file_path, offset, count) {
                Ok(rep) => println!("{}", serde_json::to_string(&rep).unwrap()),
                Err(e) => {
                    eprintln!("Disasm error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "hex" => {
            if args.len() < 3 {
                eprintln!("Usage: megadaemon-core hex <file> [offset] [length]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let offset = args.get(3).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            let length = args.get(4).and_then(|s| s.parse::<usize>().ok()).unwrap_or(256);
            match disasm::inspect_hex(file_path, "ARM64/Retro", offset, length) {
                Ok(rep) => println!("{}", serde_json::to_string(&rep).unwrap()),
                Err(e) => {
                    eprintln!("Hex error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "deobfuscate" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core deobfuscate <plat> <file>");
                std::process::exit(1);
            }
            let rep = deobfuscate::deobfuscate_binary(&args[2], &args[3], 32);
            println!("{}", serde_json::to_string(&rep).unwrap());
        }

        "dumper" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core dumper <plat> <file>");
                std::process::exit(1);
            }
            let rep = deobfuscate::extract_assets_scan(&args[2], &args[3]);
            println!("{}", serde_json::to_string(&rep).unwrap());
        }

        "entropy" => {
            if args.len() < 3 {
                eprintln!("Usage: megadaemon-core entropy <file> [block_size]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let block_size = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1024);

            let mut f = match File::open(file_path) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Error opening file: {}", e);
                    std::process::exit(1);
                }
            };
            let mut data = Vec::new();
            if let Err(e) = f.read_to_end(&mut data) {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1);
            }

            let res = entropy::scan_entropy(&data, block_size);
            println!("{}", serde_json::to_string(&res).unwrap());
        }

        "pattern" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core pattern <file> <pattern_str> [max_matches]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let pat_str = &args[3];
            let max_matches = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(128);

            let mut f = match File::open(file_path) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Error opening file: {}", e);
                    std::process::exit(1);
                }
            };
            let mut data = Vec::new();
            if let Err(e) = f.read_to_end(&mut data) {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1);
            }

            match pattern::scan_pattern(&data, pat_str, max_matches) {
                Ok(res) => println!("{}", serde_json::to_string(&res).unwrap()),
                Err(err) => {
                    eprintln!("Pattern error: {}", err);
                    std::process::exit(1);
                }
            }
        }

        "pointers" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core pointers <file> <base_vaddr_hex> [is_be] [max]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let base_hex = args[3].trim_start_matches("0x");
            let base_vaddr = u32::from_str_radix(base_hex, 16).unwrap_or(0);
            let is_be = args.get(4).map(|s| s == "1" || s == "true").unwrap_or(false);
            let max_res = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(128);

            let mut f = match File::open(file_path) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Error opening file: {}", e);
                    std::process::exit(1);
                }
            };
            let mut data = Vec::new();
            if let Err(e) = f.read_to_end(&mut data) {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1);
            }

            let res = pointers::scan_pointers_32(&data, base_vaddr, is_be, max_res);
            println!("{}", serde_json::to_string(&res).unwrap());
        }

        "diff" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core diff <file_a> <file_b> [base_vaddr_hex] [max]");
                std::process::exit(1);
            }
            let file_a_path = &args[2];
            let file_b_path = &args[3];
            let base_hex = args.get(4).map(|s| s.trim_start_matches("0x")).unwrap_or("0");
            let base_vaddr = u32::from_str_radix(base_hex, 16).unwrap_or(0);
            let max_res = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(256);

            let mut f_a = match File::open(file_a_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error opening file_a: {}", e);
                    std::process::exit(1);
                }
            };
            let mut buf_a = Vec::new();
            f_a.read_to_end(&mut buf_a).unwrap();

            let mut f_b = match File::open(file_b_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error opening file_b: {}", e);
                    std::process::exit(1);
                }
            };
            let mut buf_b = Vec::new();
            f_b.read_to_end(&mut buf_b).unwrap();

            let res = snapshot::diff_snapshots(&buf_a, &buf_b, base_vaddr, max_res);
            println!("{}", serde_json::to_string(&res).unwrap());
        }

        "shm-status" => match shm::read_shm_status() {
            Ok(hdr) => println!("{}", serde_json::to_string(&hdr).unwrap()),
            Err(e) => {
                eprintln!("SHM Error: {}", e);
                std::process::exit(1);
            }
        },

        "shm-frame-bmp" => {
            let bmp_bytes = match shm::read_shm_status() {
                Ok(hdr) if hdr.is_active => {
                    let mut file = File::open(shm::SHM_PATH).unwrap();
                    let mut buf = vec![0u8; shm::SHM_SIZE];
                    file.read_exact(&mut buf).unwrap();
                    let raw_rgb = &buf[32824..32824 + (hdr.width as usize * hdr.height as usize * 3)];
                    shm::encode_bmp_24(hdr.width as u32, hdr.height as u32, raw_rgb)
                }
                _ => shm::generate_tactical_hud_bmp(682, 448),
            };
            io::stdout().write_all(&bmp_bytes).unwrap();
        }

        cmd => {
            eprintln!("Unknown command: {}", cmd);
            print_usage();
            std::process::exit(1);
        }
    }
}
