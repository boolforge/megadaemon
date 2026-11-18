mod entropy;
mod pattern;
mod pointers;
mod shm;
mod snapshot;

use std::env;
use std::fs::File;
use std::io::{self, Read, Write};

fn print_usage() {
    eprintln!("Usage: megadaemon-core <command> [args...]");
    eprintln!("Commands:");
    eprintln!("  entropy <file> [block_size]");
    eprintln!("  pattern <file> <pattern_str> [max_matches]");
    eprintln!("  pointers <file> <base_vaddr_hex> [is_be] [max]");
    eprintln!("  diff <file_a> <file_b> [base_vaddr_hex] [max]");
    eprintln!("  shm-status");
    eprintln!("  shm-frame-bmp");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
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
