use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RomEntry {
    pub path: String,
    pub filename: String,
    pub size_bytes: u64,
    pub readable: bool,
    pub arch: String,
    pub platform_name: String,
    pub ghidra_c_path: Option<String>,
    pub ghidra_c_size: u64,
    pub ghidra_asm_path: Option<String>,
    pub ghidra_asm_size: u64,
}

static ROM_CACHE: OnceLock<Mutex<(u64, HashMap<String, RomEntry>)>> = OnceLock::new();

const SEARCH_ROOTS: &[&str] = &[
    "/data/user/0/com.agy/files/re-research/bin",
    "/storage/emulated/0/Download/Snatcher-Collection",
    "/data/user/0/com.agy/files/megadaemon/extracted",
    "/data/user/0/com.agy/files/snatcher-tools",
    "/data/user/0/com.agy/files/scummvm",
];

pub fn platform_metadata(plat: &str) -> (&'static str, &'static str, &'static [&'static str], &'static str, &'static str) {
    match plat {
        "ps1" => (
            "PlayStation 1",
            "MIPS R3000A @ 33.86MHz",
            &["ps1_slps.bin", "SLPS_001.54", "snatcher.bin", ".chd"],
            "/data/user/0/com.agy/files/re-research/out/ps1.c",
            "/data/user/0/com.agy/files/re-research/out/ps1.asm",
        ),
        "saturn" => (
            "Sega Saturn",
            "Dual Hitachi SH-2 @ 28.6MHz",
            &["saturn_main.bin", "MAIN_L.BIN", "0.BIN", "DATA_A0.BIN"],
            "/data/user/0/com.agy/files/re-research/out/saturn_main.c",
            "/data/user/0/com.agy/files/re-research/out/saturn_main.asm",
        ),
        "pce" => (
            "PC Engine CD-ROM²",
            "HuC6280 @ 7.16MHz",
            &["pce_overlay.bin", "pce_boot.bin", "scene_06_sec0158_bank16.bin", "Track 02"],
            "/data/user/0/com.agy/files/re-research/out/pce_overlay.c",
            "/data/user/0/com.agy/files/re-research/out/pce_overlay.asm",
        ),
        "msx2" => (
            "MSX2",
            "Zilog Z80 @ 3.58MHz + V9938",
            &["msx2_com.bin", "SNATCHER.COM", "SNATCHER.FNT"],
            "/data/user/0/com.agy/files/re-research/out/msx2_com.c",
            "/data/user/0/com.agy/files/re-research/out/msx2_com.asm",
        ),
        "pc88" => (
            "NEC PC-8801mkIISR",
            "Zilog Z80 @ 4MHz + YM2608 OPNA",
            &["pc88_boot.bin", "pc88_linear.img", "Disk 1 of 5", ".d88"],
            "/data/user/0/com.agy/files/re-research/out/pc88_boot.c",
            "/data/user/0/com.agy/files/re-research/out/pc88_boot.asm",
        ),
        "segacd" => (
            "Sega CD (Mega-CD)",
            "Motorola 68000 @ 12.5MHz Sub-CPU",
            &["scd_subcode.bin", "SUBCODE.BIN", "Snatcher_Spanish.chd"],
            "/data/user/0/com.agy/files/re-research/out/scd_subcode.c",
            "/data/user/0/com.agy/files/re-research/out/scd_subcode.asm",
        ),
        _ => (
            "ScummVM Core",
            "ARMv8.2-A Native C++/Rust",
            &["scummvm"],
            "",
            "",
        ),
    }
}

fn walk_find(dir: &Path, needle: &str, depth: usize) -> Option<PathBuf> {
    if depth > 6 || !dir.exists() {
        return None;
    }
    let entries = fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();
    let needle_low = needle.to_lowercase();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                if fname.to_lowercase().contains(&needle_low) {
                    return Some(path);
                }
            }
        } else if path.is_dir() {
            subdirs.push(path);
        }
    }

    for sub in subdirs {
        if let Some(found) = walk_find(&sub, needle, depth + 1) {
            return Some(found);
        }
    }
    None
}

pub fn locate_all(force_refresh: bool) -> HashMap<String, RomEntry> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mutex = ROM_CACHE.get_or_init(|| Mutex::new((0, HashMap::new())));
    if let Ok(guard) = mutex.lock() {
        if !force_refresh && !guard.1.is_empty() && now.saturating_sub(guard.0) < 60 {
            return guard.1.clone();
        }
    }

    let mut discovered = HashMap::new();
    for &plat in &["ps1", "saturn", "pce", "msx2", "pc88", "segacd"] {
        let (plat_name, arch, needles, c_path, asm_path) = platform_metadata(plat);
        let mut found_path: Option<PathBuf> = None;

        // Check needles in priority order across SEARCH_ROOTS
        'outer: for &needle in needles {
            for &root in SEARCH_ROOTS {
                let root_path = Path::new(root);
                if let Some(p) = walk_find(root_path, needle, 0) {
                    found_path = Some(p);
                    break 'outer;
                }
            }
        }

        if let Some(full) = found_path {
            let size_bytes = fs::metadata(&full).map(|m| m.len()).unwrap_or(0);
            let readable = fs::File::open(&full).is_ok();
            let filename = full
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown.bin")
                .to_string();

            let (ghidra_c_path, ghidra_c_size) = if !c_path.is_empty() && Path::new(c_path).exists() {
                (Some(c_path.to_string()), fs::metadata(c_path).map(|m| m.len()).unwrap_or(0))
            } else {
                (None, 0)
            };

            let (ghidra_asm_path, ghidra_asm_size) = if !asm_path.is_empty() && Path::new(asm_path).exists() {
                (Some(asm_path.to_string()), fs::metadata(asm_path).map(|m| m.len()).unwrap_or(0))
            } else {
                (None, 0)
            };

            discovered.insert(
                plat.to_string(),
                RomEntry {
                    path: full.to_string_lossy().into_owned(),
                    filename,
                    size_bytes,
                    readable,
                    arch: arch.to_string(),
                    platform_name: plat_name.to_string(),
                    ghidra_c_path,
                    ghidra_c_size,
                    ghidra_asm_path,
                    ghidra_asm_size,
                },
            );
        }
    }

    if let Ok(mut guard) = mutex.lock() {
        *guard = (now, discovered.clone());
    }
    discovered
}

pub fn resolve_binary(plat_key: &str) -> Option<RomEntry> {
    let key = plat_key.to_lowercase();
    let all = locate_all(false);
    all.get(&key).cloned()
}
