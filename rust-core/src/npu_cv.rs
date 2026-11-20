//! MegaDaemon v13.0 Zenith — Exynos 2100 NPU + Mali-G78 + Native Rust VLA Computer Vision Pipeline
//! Performs sub-millisecond frame-delta motion gating, Sobel edge energy, HUD/dialogue ROI extraction,
//! dominant palette mining, and hardware accelerator telemetry (Exynos 2100 NPU EDEN / Mali-G78 / KleidiAI).

use crate::{npu_bridge, npu_nnapi, shm};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
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
    /// Backend REAL que ejecutó el análisis de bordes:
    /// "nnapi:accelerator" | "nnapi:gpu" | "nnapi:cpu" | "cpu-reference".
    pub vision_backend: String,
    /// `true` sólo si la ejecución fue realmente sobre hardware NNAPI.
    pub vision_executed_on_hardware: bool,
    /// Motivo del fallback a CPU (vacío si no hubo fallback).
    pub vision_fallback_reason: String,
    /// Latencia del análisis de bordes por sí solo (sin I/O de frame).
    pub vision_latency_ms: f64,
    /// Algoritmo usado: la tubería es la misma en NPU y en CPU.
    pub edge_energy_model: String,
    pub nnapi_device_count: u32,
    pub nnapi_devices: Vec<npu_nnapi::NnapiDevice>,
    /// `true` si el frame cambió mientras se leía (análisis potencialmente
    /// inconsistente; se declara en vez de esconderlo).
    pub frame_torn: bool,
    /// Contador de frame del producer (para saber si la imagen es nueva).
    pub frame_counter: u32,
    /// `true` si este es el primer análisis (no hay frame previo con el que
    /// comparar, así que el delta es 0 por definición, no medido).
    pub first_frame: bool,
    /// Mapa de bordes reducido (máx. 64 columnas) para dibujar el mapa de
    /// calor en la UI.  Valores 0..127 por celda (energía por canal sumada).
    pub edge_map: Vec<u8>,
    pub edge_map_w: u32,
    pub edge_map_h: u32,
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
    /// Cargas de trabajo que el repo EJECUTA de verdad en el NPU (v13.2).
    pub npu_workloads_implemented: Vec<String>,
    /// Cargas de trabajo técnicamente posibles con un modelo cuantizado.
    pub npu_workloads_possible: Vec<String>,
    /// Cargas de trabajo IMPOSIBLES en el NPU de Exynos (con motivo).
    pub npu_workloads_impossible: Vec<String>,
}

static PREV_LUMA: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();

pub fn analyze_vla_frame_and_accel() -> NpuCvReport {
    let t0 = Instant::now();

    // v13.2: una sola lectura coherente del frame (antes se leía el fichero
    // SHM dos veces: 2 MB por análisis) y con detección de "frame a medio
    // escribir" (el productor escribe mientras leemos).
    let snap = shm::read_frame_snapshot().ok();
    let torn = snap.as_ref().map(|s| s.torn).unwrap_or(false);
    let frame_counter = snap.as_ref().map(|s| s.header.frame_counter).unwrap_or(0);
    let (w, h, rgb) = match snap {
        Some(s) if s.header.width > 0 && s.header.height > 0 => (
            s.header.width as usize,
            s.header.height as usize,
            s.rgb,
        ),
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
    // `first_frame_seen` se inicializa dentro del bloque (siempre se asigna).
    let first_frame_seen;
    let delta_l1 = {
        let mut prev = prev_lock.lock().unwrap_or_else(|p| p.into_inner());
        // v13.2 (HW-11): sin frame previo NO hay delta que medir.  Antes se
        // fabricaba 12,5 y `scene_changed: true`, es decir, se inventaba
        // movimiento en el primer análisis (y se disparaba el VLM de verdad).
        let (d, first_frame) = if prev.len() == luma.len() && !luma.is_empty() {
            let sum: u64 = luma
                .iter()
                .zip(prev.iter())
                .map(|(&a, &b)| (a as i32 - b as i32).unsigned_abs() as u64)
                .sum();
            (sum as f64 / luma.len() as f64, false)
        } else {
            (0.0, true)
        };
        *prev = luma.clone();
        first_frame_seen = first_frame;
        (d * 100.0).round() / 100.0
    };

    // ── Detección de estructura (bordes) ───────────────────────────────────
    // v13.2: la tubería de bordes pasa por el backend NNAPI real (grafo INT8
    // conv3x3 Sobel + relu + maxpool2x2).  Si no hay NNAPI o el driver
    // rechaza el modelo, se usa la REFERENCIA CPU con la MISMA aritmética
    // entera, de modo que el resultado es comparable y sólo cambia quién
    // ejecuta.  La telemetría dice siempre qué backend corrió.
    let (edge, edge_exec) = npu_nnapi::analyze_edges(&luma, w as u32, h as u32);
    let mean_sobel = edge.mean_energy;
    let rois = vec![
        RoiBox {
            label: "TOP_HUD_STATUS".into(),
            x: 0,
            y: 0,
            w: w as u32,
            h: (h / 4) as u32,
            edge_density: ((edge.band_density(0) * 1000.0).round()) / 1000.0,
        },
        RoiBox {
            label: "MAIN_VIEWPORT_SCENE".into(),
            x: 0,
            y: (h / 4) as u32,
            w: w as u32,
            h: ((h * 5) / 12) as u32,
            edge_density: ((edge.band_density(1) * 1000.0).round()) / 1000.0,
        },
        RoiBox {
            label: "DIALOGUE_TEXT_BOX".into(),
            x: 0,
            y: ((h * 2) / 3) as u32,
            w: w as u32,
            h: (h - (h * 2) / 3) as u32,
            edge_density: ((edge.band_density(2) * 1000.0).round()) / 1000.0,
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
        npu_workloads_implemented: vec![
            "INT8 conv3x3(Sobel X/Y) + relu + maxpool2x2 sobre el frame SHM via NNAPI (deteccion de estructura HUD/dialogo)".into(),
            "Muestreo de paleta dominante y delta L1 frame-a-frame en CPU (barato, no merece offload)".into(),
        ],
        npu_workloads_possible: vec![
            "MobileNetV3/SSD/YOLO INT8 via NNAPI: requiere un modelo cuantizado por tensor + NMS en CPU (NO implementado; falta el .tflite/.int8)".into(),
            "NPU para el LLM: NO. NNAPI no soporta atencion ni formas dinamicas y llama.cpp no tiene backend Exynos".into(),
        ],
        npu_workloads_impossible: vec![
            "LLM autoregresivo (prefill/decode) en el NPU del Exynos 2100: sin ops de atencion en NNAPI y sin backend NPU en llama.cpp".into(),
            "Vulkan/Mali-G78 para el LLM: medido 2x mas lento en decode que la CPU en el Galaxy S21 (Exynos 2100)".into(),
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
        vision_backend: edge_exec.backend.clone(),
        vision_executed_on_hardware: edge_exec.executed_on_hardware,
        vision_fallback_reason: edge_exec.fallback_reason.clone(),
        vision_latency_ms: edge_exec.latency_ms,
        edge_energy_model: "int8 conv3x3-sobel -> relu -> maxpool2x2 (NHWC)".into(),
        nnapi_device_count: edge_exec.nnapi_device_count,
        nnapi_devices: edge_exec.nnapi_devices.clone(),
        frame_torn: torn,
        frame_counter,
        first_frame: first_frame_seen,
        edge_map: downsample_edge_map(&edge, 64),
        edge_map_w: downsample_edge_map_w(&edge, 64),
        edge_map_h: downsample_edge_map_h(&edge, 64),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Visión continua: el NPU trabaja en continuo, no sólo bajo demanda
// ─────────────────────────────────────────────────────────────────────────────

use crate::power_policy;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Estado observable del worker de visión continua.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionStatus {
    pub running: bool,
    pub frames: u64,
    pub torn_frames: u64,
    pub last_frame_counter: u32,
    pub last_backend: String,
    pub last_latency_ms: f64,
    pub computed_hz: f64,
    pub power_policy: String,
    pub interval_ms: u64,
    pub started_at_us: u64,
    pub last_error: String,
}

#[derive(Debug, Default)]
struct VisionInner {
    frames: u64,
    computed_hz: f64,
    torn_frames: u64,
    last_frame_counter: u32,
    last_backend: String,
    last_latency_ms: f64,
    started_at_us: u64,
    last_error: String,
}

static VISION_STATE: OnceLock<Mutex<VisionInner>> = OnceLock::new();
static VISION_RUN: OnceLock<AtomicBool> = OnceLock::new();
static VISION_HANDLE: OnceLock<Mutex<Option<std::thread::JoinHandle<()>>>> = OnceLock::new();

fn vision_state() -> &'static Mutex<VisionInner> {
    VISION_STATE.get_or_init(|| Mutex::new(VisionInner::default()))
}

fn vision_run_flag() -> &'static AtomicBool {
    VISION_RUN.get_or_init(|| AtomicBool::new(false))
}

/// Arranca (o reanuda) el análisis continuo de visión.
///
/// Antes el análisis de visión sólo se ejecutaba **bajo demanda** (al pulsar
/// un botón o al abrir un endpoint): el NPU estaba ocioso el 99 % del tiempo.
/// Con el worker, el grafo NNAPI se ejecuta de forma sostenida, respetando la
/// política de energía: 2 Hz en operación completa, 0,2 Hz en ahorro y sólo
/// lectura de contador en modo crítico.
pub fn start_continuous() {
    let flag = vision_run_flag();
    if flag.swap(true, Ordering::SeqCst) {
        return;
    }
    let handle = std::thread::Builder::new()
        .name("megadaemon-vision".into())
        .spawn(|| vision_loop())
        .ok();
    if let Some(h) = handle {
        if let Ok(mut slot) = VISION_HANDLE.get_or_init(|| Mutex::new(None)).lock() {
            *slot = Some(h);
        }
    }
    let mut st = vision_state().lock().unwrap_or_else(|p| p.into_inner());
    st.started_at_us = now_us();
}

/// Detiene el análisis continuo.
pub fn stop_continuous() {
    vision_run_flag().store(false, Ordering::SeqCst);
}

fn now_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

fn vision_loop() {
    let mut last_frames = 0u64;
    let mut last_measure_us = now_us();
    loop {
        if !vision_run_flag().load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(200));
            continue;
        }
        let (_, pol) = power_policy::current_policy();
        // Cadencia según la política de energía: el trabajo pesado se espacia.
        let interval_ms = match pol.name {
            "performance" => 500,
            "saver" | "thermal" => 5_000,
            _ => 5_000,
        };
        std::thread::sleep(std::time::Duration::from_millis(interval_ms));
        if !vision_run_flag().load(Ordering::SeqCst) {
            continue;
        }
        // En modo crítico no se toca el frame: sólo se lee el contador.
        if pol.name == "critical" {
            if let Ok(hdr) = shm::read_shm_status() {
                let mut st = vision_state().lock().unwrap_or_else(|p| p.into_inner());
                st.last_frame_counter = hdr.frame_counter;
            }
            continue;
        }
        let rep = analyze_vla_frame_and_accel();
        let mut st = vision_state().lock().unwrap_or_else(|p| p.into_inner());
        st.frames += 1;
        if rep.frame_torn {
            st.torn_frames += 1;
        }
        st.last_frame_counter = rep.frame_counter;
        st.last_backend = rep.vision_backend.clone();
        st.last_latency_ms = rep.cv_latency_ms;
        if rep.vision_fallback_reason.len() > 0 && st.last_error.is_empty() {
            st.last_error = rep.vision_fallback_reason.clone();
        }
        drop(st);
        let now = now_us();
        if now.saturating_sub(last_measure_us) >= 2_000_000 {
            let mut st = vision_state().lock().unwrap_or_else(|p| p.into_inner());
            let dt = (now - last_measure_us) as f64 / 1_000_000.0;
            let df = (st.frames - last_frames) as f64;
            st.computed_hz = if dt > 0.0 { (df / dt * 100.0).round() / 100.0 } else { 0.0 };
            last_frames = st.frames;
            last_measure_us = now;
        }
    }
}

/// Instantánea del estado del worker de visión.
pub fn vision_status() -> VisionStatus {
    let (_, pol) = power_policy::current_policy();
    let st = vision_state().lock().unwrap_or_else(|p| p.into_inner());
    VisionStatus {
        running: vision_run_flag().load(Ordering::SeqCst),
        frames: st.frames,
        torn_frames: st.torn_frames,
        last_frame_counter: st.last_frame_counter,
        last_backend: st.last_backend.clone(),
        last_latency_ms: st.last_latency_ms,
        computed_hz: st.computed_hz,
        power_policy: pol.name.to_string(),
        interval_ms: match pol.name {
            "performance" => 500,
            _ => 5_000,
        },
        started_at_us: st.started_at_us,
        last_error: st.last_error.clone(),
    }
}

/// Reduce el mapa pooled a como mucho `max_w` columnas para la UI.
fn downsample_edge_map(edge: &npu_nnapi::EdgeResult, max_w: u32) -> Vec<u8> {
    let (pw, ph) = (edge.pooled_w as usize, edge.pooled_h as usize);
    if pw == 0 || ph == 0 {
        return Vec::new();
    }
    let step = ((pw + max_w as usize - 1) / max_w as usize).max(1);
    let mut out = Vec::with_capacity((pw / step) * ph);
    for y in 0..ph {
        for x in (0..pw).step_by(step) {
            let base = (y * pw + x) * 2;
            let v = edge.pooled.get(base).copied().unwrap_or(0) as i16
                + edge.pooled.get(base + 1).copied().unwrap_or(0) as i16;
            out.push(v.clamp(0, 255) as u8);
        }
    }
    out
}

fn downsample_edge_map_w(edge: &npu_nnapi::EdgeResult, max_w: u32) -> u32 {
    let pw = edge.pooled_w as usize;
    if pw == 0 {
        return 0;
    }
    let step = ((pw + max_w as usize - 1) / max_w as usize).max(1);
    ((pw + step - 1) / step) as u32
}

fn downsample_edge_map_h(edge: &npu_nnapi::EdgeResult, _max_w: u32) -> u32 {
    edge.pooled_h
}
