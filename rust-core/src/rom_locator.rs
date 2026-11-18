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
}

static ROM_CACHE: OnceLock<Mutex<(u64, HashMap<String, RomEntry>)>> = OnceLock::new();

const SEARCH_ROOTS: &[&str] = &[
    "/storage/emulated/0/Download/Snatcher-Collection",
    "/storage/emulated/0/Download",
    "/data/user/0/com.agy/files/megadaemon/extracted",
    "/data/user/0/com.agy/files/scummvm",
];

pub fn platform_metadata(plat: &str) -> (&'static str, &'static str, &'static [&'static str]) {
    match plat {
        "ps1" => (
            "PlayStation 1",
            "MIPS R3000A @ 33.86MHz",
            &["SLPS_001.54", "SLPS", ".chd", "snatcher.bin"],
        ),
        "saturn" => (
            "Sega Saturn",
            "Dual Hitachi SH-2 @ 28.6MHz",
            &["MAIN_L.BIN", "0.BIN", "DATA_A0.BIN"],
        ),
        "pce" => (
            "PC Engine CD-ROM²",
            "HuC6280 @ 7.16MHz",
            &["scene_06_sec0158_bank16.bin", "Track 02", "pce_mode1_clean.iso"],
        ),
        "msx2" => (
            "MSX2",
            "Zilog Z80 @ 3.58MHz + V9938",
            &["SNATCHER.COM", "SNATCHER.FNT", "Disk 1 of 3.dsk"],
        ),
        "pc88" => (
            "NEC PC-8801mkIISR",
            "Zilog Z80 @ 4MHz + YM2608 OPNA",
            &["pc88_linear.img", "Disk 1 of 5", "Disk1-Program_linear.img", ".d88"],
        ),
        "segacd" => (
            "Sega CD (Mega-CD)",
            "Motorola 68000 @ 12.5MHz Sub-CPU",
            &["SUBCODE.BIN", "Snatcher_Spanish.chd", "segacd"],
        ),
        _ => ("ScummVM Core", "ARMv8.2-A Native C++/Rust", &["scummvm"]),
    }
}

fn walk_find(dir: &Path, needles: &[&str], depth: usize) -> Option<PathBuf> {
    if depth > 6 || !dir.exists() {
        return None;
    }
    let entries = fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                let lower = fname.to_lowercase();
                for &needle in needles {
                    if lower.contains(&needle.to_lowercase()) {
                        return Some(path);
                    }
                }
            }
        } else if path.is_dir() {
            subdirs.push(path);
        }
    }

    for sub in subdirs {
        if let Some(found) = walk_find(&sub, needles, depth + 1) {
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
        let (plat_name, arch, needles) = platform_metadata(plat);
        let mut found_path: Option<PathBuf> = None;

        for &root in SEARCH_ROOTS {
            let root_path = Path::new(root);
            if let Some(p) = walk_find(root_path, needles, 0) {
                found_path = Some(p);
                break;
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
            discovered.insert(
                plat.to_string(),
                RomEntry {
                    path: full.to_string_lossy().into_owned(),
                    filename,
                    size_bytes,
                    readable,
                    arch: arch.to_string(),
                    platform_name: plat_name.to_string(),
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
