//! MegaDaemon v13.0 Zenith — Exynos 2100 NPU + Mali-G78 + Native Rust VLA Computer Vision Pipeline
//! Performs sub-millisecond frame-delta motion gating, Sobel edge energy, HUD/dialogue ROI extraction,
//! dominant palette mining, and hardware accelerator telemetry (Exynos 2100 NPU EDEN / Mali-G78 / KleidiAI).

use crate::{npu_bridge, shm};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoiBox {
    pub label: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub edge_density: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpuCvReport {
    pub frame_width: u32,
    pub frame_height: u32,
    pub frame_delta_l1: f64,
    pub scene_changed: bool,
    pub sobel_mean_energy: f64,
    pub hud_rois: Vec<RoiBox>,
    pub dominant_colors_hex: Vec<String>,
    pub cv_latency_ms: f64,
    pub hardware: HardwareAccelMatrix,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareAccelMatrix {
    pub npu_vertex10: bool,
    pub npu_eden_dlopen: bool,
    pub npu_dsp_lib: bool,
    pub mali_g78_opencl_lib: bool,
    pub mali_g78_vulkan_hal: bool,
    pub kleidiai_sdot: bool,
    pub cpu_freqs_mhz: Vec<u32>,
    pub cpu_temp_c: f32,
    pub battery_temp_c: f32,
    pub recommended_npu_workloads: Vec<String>,
}

static PREV_LUMA: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();

pub fn analyze_vla_frame_and_accel() -> NpuCvReport {
    let t0 = Instant::now();
    let (w, h, rgb) = match shm::read_shm_status() {
        Ok(hdr) if hdr.is_active && hdr.width > 0 && hdr.height > 0 => {
            let ww = hdr.width as usize;
            let hh = hdr.height as usize;
            if let Ok(mut f) = File::open(shm::SHM_PATH) {
                let mut buf = vec![0u8; shm::SHM_SIZE];
                if f.read_exact(&mut buf).is_ok() {
                    let end = (32824 + ww * hh * 3).min(buf.len());
                    (ww, hh, buf[32824..end].to_vec())
                } else {
                    (320, 224, vec![16u8; 320 * 224 * 3])
                }
            } else {
                (320, 224, vec![16u8; 320 * 224 * 3])
            }
        }
        _ => (320, 224, vec![16u8; 320 * 224 * 3]),
    };

    let pixels = w * h;
    let mut luma = vec![0u8; pixels];
    let mut color_bins = [0u32; 64]; // 2 bits per R,G,B -> 64 quantized buckets

    for i in 0..pixels {
        let base = i * 3;
        if base + 2 < rgb.len() {
            let r = rgb[base];
            let g = rgb[base + 1];
            let b = rgb[base + 2];
            luma[i] = ((r as u32 * 77 + g as u32 * 150 + b as u32 * 29) >> 8) as u8;
            let bin = (((r >> 6) as usize) << 4) | (((g >> 6) as usize) << 2) | ((b >> 6) as usize);
            color_bins[bin] += 1;
        }
    }

    // Frame-to-frame L1 delta (motion gating for VLM call savings)
    let prev_lock = PREV_LUMA.get_or_init(|| Mutex::new(Vec::new()));
    let delta_l1 = {
        let mut prev = prev_lock.lock().unwrap();
        let d = if prev.len() == luma.len() && !luma.is_empty() {
            let sum: u64 = luma
                .iter()
                .zip(prev.iter())
                .map(|(&a, &b)| (a as i32 - b as i32).unsigned_abs() as u64)
                .sum();
            sum as f64 / luma.len() as f64
        } else {
            12.5
        };
        *prev = luma.clone();
        (d * 100.0).round() / 100.0
    };

    // Fast Sobel gradient magnitude on interior pixels (sampled stride 2 for sub-ms mobile perf)
    let mut sobel_sum = 0.0f64;
    let mut sobel_cnt = 0u64;
    let mut top_edges = 0u64;
    let mut mid_edges = 0u64;
    let mut bot_edges = 0u64;

    if w >= 4 && h >= 4 {
        for y in (1..h - 1).step_by(2) {
            for x in (1..w - 1).step_by(2) {
                let p = |xx: usize, yy: usize| luma[yy * w + xx] as i32;
                let gx = -p(x - 1, y - 1) + p(x + 1, y - 1) - 2 * p(x - 1, y) + 2 * p(x + 1, y)
                    - p(x - 1, y + 1)
                    + p(x + 1, y + 1);
                let gy = -p(x - 1, y - 1) - 2 * p(x, y - 1) - p(x + 1, y - 1) + p(x - 1, y + 1)
                    + 2 * p(x, y + 1)
                    + p(x + 1, y + 1);
                let mag = (gx.abs() + gy.abs()) as u64;
                sobel_sum += mag as f64;
                sobel_cnt += 1;
                if mag > 80 {
                    if y < h / 4 {
                        top_edges += 1;
                    } else if y > (h * 2) / 3 {
                        bot_edges += 1;
                    } else {
                        mid_edges += 1;
                    }
                }
            }
        }
    }

    let mean_sobel = if sobel_cnt > 0 {
        ((sobel_sum / sobel_cnt as f64) * 100.0).round() / 100.0
    } else {
        0.0
    };

    let denom = (sobel_cnt.max(1) as f64) / 3.0;
    let rois = vec![
        RoiBox {
            label: "TOP_HUD_STATUS".into(),
            x: 0,
            y: 0,
            w: w as u32,
            h: (h / 4) as u32,
            edge_density: ((top_edges as f64 / denom) * 1000.0).round() / 1000.0,
        },
        RoiBox {
            label: "MAIN_VIEWPORT_SCENE".into(),
            x: 0,
            y: (h / 4) as u32,
            w: w as u32,
            h: ((h * 5) / 12) as u32,
            edge_density: ((mid_edges as f64 / denom) * 1000.0).round() / 1000.0,
        },
        RoiBox {
            label: "DIALOGUE_TEXT_BOX".into(),
            x: 0,
            y: ((h * 2) / 3) as u32,
            w: w as u32,
            h: (h - (h * 2) / 3) as u32,
            edge_density: ((bot_edges as f64 / denom) * 1000.0).round() / 1000.0,
        },
    ];

    let mut ranked_bins: Vec<(usize, u32)> = color_bins.iter().enumerate().map(|(i, &c)| (i, c)).collect();
    ranked_bins.sort_by(|a, b| b.1.cmp(&a.1));
    let dominant: Vec<String> = ranked_bins
        .iter()
        .take(5)
        .map(|&(bin, _)| {
            let r = (((bin >> 4) & 3) * 85) as u8;
            let g = (((bin >> 2) & 3) * 85) as u8;
            let b = ((bin & 3) * 85) as u8;
            format!("#{:02X}{:02X}{:02X}", r, g, b)
        })
        .collect();

    let npu = npu_bridge::probe_exynos_npu();
    let hw = HardwareAccelMatrix {
        npu_vertex10: npu.vs4l_present,
        npu_eden_dlopen: npu.eden_runtime.dlopen_ok,
        npu_dsp_lib: Path::new("/vendor/lib64/libeden_ud_dsp.so").exists(),
        mali_g78_opencl_lib: Path::new("/vendor/lib64/libOpenCL.so").exists(),
        mali_g78_vulkan_hal: Path::new("/vendor/lib64/hw/vulkan.mali.so").exists(),
        kleidiai_sdot: npu.kleidiai_dotprod_active,
        cpu_freqs_mhz: npu.cpu_freqs_mhz,
        cpu_temp_c: npu.cpu_temp_c,
        battery_temp_c: npu.battery_temp_c,
        recommended_npu_workloads: vec![
            "INT8 Vision CNN (YOLO/MobileNetV3 sprite & HUD ROI detection @ 120 FPS on EDEN NPU)".into(),
            "INT8 PaddleOCR / CRNN retro dialogue text extraction (zero CPU thermal load)".into(),
            "INT8 CLIP / SigLIP visual embeddings for frame-to-script semantic indexing".into(),
            "INT8 Audio Whisper-Tiny / KWS speech-to-text for Sega CD / PS1 voice clips".into(),
            "CPU KleidiAI SDOT (Cortex-X1 + 3x A78) for autoregressive LLM decode (memory-bandwidth bound)".into(),
        ],
    };

    NpuCvReport {
        frame_width: w as u32,
        frame_height: h as u32,
        frame_delta_l1: delta_l1,
        scene_changed: delta_l1 > 1.5,
        sobel_mean_energy: mean_sobel,
        hud_rois: rois,
        dominant_colors_hex: dominant,
        cv_latency_ms: (t0.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0,
        hardware: hw,
    }
}
