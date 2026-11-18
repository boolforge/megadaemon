use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedProjectNode {
    pub id: String,
    pub name: String,
    pub path: String,
    pub present: bool,
    pub file_count: usize,
    pub total_bytes: u64,
    pub role: String,
    pub key_artifacts: Vec<String>,
    pub integration_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemUnifierReport {
    pub status: String,
    pub unifier_version: String,
    pub total_projects_unified: usize,
    pub total_ecosystem_files: usize,
    pub total_ecosystem_bytes: u64,
    pub ghidra_corpus_total_bytes: u64,
    pub projects: Vec<UnifiedProjectNode>,
    pub rust_clients_compiled: Vec<String>,
    pub engine: String,
}

fn scan_dir_stats(dir: &Path, max_depth: usize) -> (usize, u64) {
    if max_depth == 0 || !dir.exists() {
        return (0, 0);
    }
    let mut count = 0usize;
    let mut bytes = 0u64;
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if fname == ".git" || fname == "target" || fname == "__pycache__" {
                continue;
            }
            if p.is_file() {
                count += 1;
                bytes += fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            } else if p.is_dir() {
                let (c, b) = scan_dir_stats(&p, max_depth - 1);
                count += c;
                bytes += b;
            }
        }
    }
    (count, bytes)
}

pub fn scan_unified_ecosystem() -> EcosystemUnifierReport {
    let specs = [
        (
            "megadaemon",
            "MegaDaemon v12.0 Omega Core & Native Rust Cockpit",
            "/data/user/0/com.agy/files/megadaemon",
            "Orquestador central en Rust ARM64 (17 módulos), Servidor HTTP Cockpit (:3000), SEERE v12 GBNF y Puente NPU Exynos 2100",
            vec!["rust-core/src/cockpit_server.rs", "rust-core/src/seere_v11.rs", "rust-core/src/npu_bridge.rs", "rust-core/src/rea_assistant.rs"],
        ),
        (
            "re-research",
            "RE-Research (Ghidra 2.7MB Decompiled C/ASM Corpus & Clean Binaries)",
            "/data/user/0/com.agy/files/re-research",
            "Corpus de descompilación Ghidra (ps1.c 935KB, scd_subcode.c 44KB, msx2_com.c 59KB) y 7 binarios ejecutables limpios",
            vec!["out/ps1.c", "out/scd_subcode.c", "out/msx2_com.c", "bin/ps1_slps.bin", "bin/saturn_main.bin", "bin/scd_subcode.bin", "Ghidra_HuC6280"],
        ),
        (
            "dumper-web",
            "Dumper-Web & Universal Media Converter (HFS+/ISO9660/ECM/Shift-JIS)",
            "/data/user/0/com.agy/files/dumper-web",
            "Extractor universal de contenedores HFS+, ISO9660, CUE/BIN, ECM, Shift-JIS y transpilador C++ nativo",
            vec!["dumper-companion.py", "snatcher_universal_converter.py", "extract_snatcher_pce.py", "depack_pc88.py", "decompile_native_cpp.py"],
        ),
        (
            "snatcher-tools",
            "Snatcher Multi-Platform Translation & ISO Patching Suite",
            "/data/user/0/com.agy/files/snatcher-tools",
            "Herramientas de reinserción de scripts, fuentes y sectores para PC Engine, PlayStation 1 y Sega Saturn",
            vec!["build.py", "pce/", "ps1/", "saturn/", "lib/"],
        ),
        (
            "sega-cd-patcher",
            "Sega-CD Snatcher Language Patcher & Spatcher",
            "/data/user/0/com.agy/files/Sega-CD-Snatcher-Language-Patcher",
            "Parcheador de idioma e inyector de subcódigos M68K para Sega CD / Mega-CD",
            vec!["Sega-CD-Snatcher-Language-Patcher", "Sega-Mega-CD-Spatcher"],
        ),
        (
            "scummvm",
            "ScummVM Target Engine & Autonomous RE Pipeline",
            "/data/user/0/com.agy/files/scummvm",
            "Motor destino C++ (engines/snatcher/) y pipeline de verificación diferencial de 16 fases",
            vec!["engines/snatcher/", "tools/re_pipeline/parasite_llm_router.py"],
        ),
        (
            "libchdr",
            "libchdr (Native C/C++ MAME CHD Decompressor)",
            "/data/user/0/com.agy/files/libchdr",
            "Biblioteca nativa de descompresión de imágenes ópticas CHD v1-v5 (LZMA/ZLIB/FLAC/CDZL)",
            vec!["include/libchdr/", "src/libchdr_chd.c"],
        ),
        (
            "rust-demon-clients",
            "Native Rust Demon Clients (SEERE, MALPHAS, UKOBACH)",
            "/data/user/0/com.agy/files/megadaemon/client-malphas",
            "Tríada de clientes TUI/CLI en Rust puro a 60 FPS conectados por SHM y TCP",
            vec!["client-seere/src/main.rs", "client-malphas/src/main.rs", "client-ukobach/src/main.rs"],
        ),
        (
            "seere-android-gpui",
            "SEERE Android & GPUI Mobile Native Clients",
            "/data/user/0/com.agy/files/seere-android",
            "Clientes nativos Android / GPUI con aceleración de GPU móvil",
            vec!["seere-android", "seere-gpui-apk", "gpui-mobile"],
        ),
    ];

    let mut projects = Vec::new();
    let mut total_files = 0usize;
    let mut total_bytes = 0u64;

    for (id, name, path_str, role, artifacts) in specs {
        let p = Path::new(path_str);
        let present = p.exists();
        let (fc, tb) = scan_dir_stats(p, 3);
        total_files += fc;
        total_bytes += tb;

        projects.push(UnifiedProjectNode {
            id: id.to_string(),
            name: name.to_string(),
            path: path_str.to_string(),
            present,
            file_count: fc,
            total_bytes: tb,
            role: role.to_string(),
            key_artifacts: artifacts.into_iter().map(|s| s.to_string()).collect(),
            integration_status: if present {
                "UNIFIED_ACTIVE_RUST_V12".to_string()
            } else {
                "MISSING".to_string()
            },
        });
    }

    let ghidra_out = Path::new("/data/user/0/com.agy/files/re-research/out");
    let (_, ghidra_bytes) = scan_dir_stats(ghidra_out, 2);

    EcosystemUnifierReport {
        status: "OK".to_string(),
        unifier_version: "12.0.0-omega".to_string(),
        total_projects_unified: projects.iter().filter(|p| p.present).count(),
        total_ecosystem_files: total_files,
        total_ecosystem_bytes: total_bytes,
        ghidra_corpus_total_bytes: ghidra_bytes,
        projects,
        rust_clients_compiled: vec![
            "megadaemon-core (v12.0 Omega Server + RE Engine)".into(),
            "client-seere (Rust Cognitive CLI)".into(),
            "client-malphas (Rust 60FPS Matrix TUI)".into(),
            "client-ukobach (Rust Boiler & Bus Monitor)".into(),
        ],
        engine: "rust-arm64-v12.0-mega-unifier".to_string(),
    }
}
