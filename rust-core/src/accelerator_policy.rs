//! MegaDaemon v13.2 — Política REAL de aceleradores.
//!
//! Sustituye al string de la v13.0 ("Prefill Batch (N>=32): Exynos NPU
//! (EDEN/NNAPI) + Mali-G78 Vulkan | Decode (N=1): KleidiAI SDOT..."), que
//! describía una política que NINGÚN módulo ejecutaba.
//!
//! # Qué se aceleró de verdad, y con qué evidencia
//!
//! | Carga | Dispositivo | Evidencia |
//! |---|---|---|
//! | LLM decode (Qwen2.5-0.5B Q4_K_M) | **CPU** (Cortex-X1 + A78, dotprod) | Galaxy S21 (Exynos 2100 / Mali-G78 MP14): CPU 46-47 tok/s vs GPU Vulkan 22,8-23,1 tok/s → la GPU es **2x más lenta** (ameva-runtime, medición física 2026-10-04) |
//! | LLM prefill | **CPU** (mismos 4 núcleos grandes) | mismo estudio: CPU 115-119 tok/s vs GPU 227 tok/s (GPU gana prefill, pero.decode pierde el doble) |
//! | Visión (bordes/estructura UI) | **NPU** (NNAPI INT8) | implementado en `npu_nnapi`; el NPU del Exynos 2100 es INT8-only (ETH Zürich AIM'21) |
//! | Visión pesada (YOLO/OCR/CLIP) | NPU posible, **NO implementado** | falta el modelo cuantizado y el NMS |
//!
//! # Reglas duras
//!
//! * El **NPU no ejecuta el LLM**: NNAPI no tiene operaciones de atención ni
//!   admite formas dinámicas; llama.cpp no tiene backend Exynos.  El
//!   precedente más cercano (RK3588, NPU con `MUL_MAT` nativo) midió decode
//!   **más lento** en NPU que en CPU por el ping-pong de tensores.
//! * **Vulkan/Mali NO para el LLM**: medido 2x más lento en decode en el
//!   propio Galaxy S21, y 8-22x más lento en otros Mali (llama.cpp#22682,
//!   rk-llama.cpp benchmarks).
//! * El LLM va a la CPU con **dotprod + i8mm** (`armv8.2-a+dotprod+i8mm`) y
//!   **KleidiAI** (`GGML_CPU_KLEIDIAI=ON`, kernels ARM para SDOT/INT8), y
//!   fijo a los núcleos grandes (4-7 = 1x X1 + 3x A78).
//! * La visión se queda en el NPU (INT8) para no competir con el LLM por los
//!   núcleos grandes.

use serde::{Deserialize, Serialize};

/// Una regla de aceleración con su veredicto y su motivo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelRule {
    /// "llm-decode", "llm-prefill", "vision", ...
    pub workload: String,
    /// "cpu", "npu", "gpu", "none"
    pub device: String,
    /// `true` si el repo implementa realmente este camino.
    pub implemented: bool,
    /// Motivo (con la medición o la razón técnica).
    pub reason: String,
    /// Cifra medida que sustenta la decisión (si existe).
    pub measured: String,
}

/// Política completa de aceleradores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceleratorPolicy {
    pub soc: String,
    pub rules: Vec<AccelRule>,
    /// Flags de compilación de llama.cpp recomendados en este SoC.
    pub llama_build_flags: Vec<String>,
    /// Flags de ejecución de llama-server.
    pub llama_server_flags: Vec<String>,
    /// Núcleos donde vive el LLM (big cluster).
    pub llm_cores: Vec<u32>,
    pub llm_threads: u32,
    /// Núcleos donde vive la visión (little cluster: no compite con el LLM).
    pub vision_cores: Vec<u32>,
    /// Versión del documento de evidencia.
    pub evidence_version: String,
}

/// Devuelve la política de aceleradores con los veredictos justificados.
pub fn policy() -> AcceleratorPolicy {
    AcceleratorPolicy {
        soc: "Samsung Exynos 2100 (S5E9840) — 1x Cortex-X1 + 3x Cortex-A78 + 4x Cortex-A55 + NPU INT8 (26 TOPS)".into(),
        rules: vec![
            AccelRule {
                workload: "llm-decode".into(),
                device: "cpu".into(),
                implemented: true,
                reason: "El NPU del Exynos 2100 es INT8-only y no tiene ops de atención; llama.cpp no tiene backend Exynos. En el Galaxy S21 (Exynos 2100) la CPU rinde 46-47 tok/s de decode frente a 22,8-23,1 tok/s de Vulkan/Mali: la GPU duplica la latencia.".into(),
                measured: "S21 Exynos2100 Qwen2.5-0.5B Q4_K_M: CPU 46-47 tok/s vs GPU 22,8-23,1 tok/s (ameva-runtime, 2026-10-04)".into(),
            },
            AccelRule {
                workload: "llm-prefill".into(),
                device: "cpu".into(),
                implemented: true,
                reason: "Vulkan/Mali gana prefill (227 vs 115-119 tok/s) pero pierde el doble en decode; el prompt del SEERE es de 187-207 tokens, así que el prefill no domina el episodio.".into(),
                measured: "S21: CPU 115-119 tok/s prompt vs GPU 227 tok/s".into(),
            },
            AccelRule {
                workload: "vision-edge-detection".into(),
                device: "npu".into(),
                implemented: true,
                reason: "Grafo NNAPI INT8 de forma fija (conv3x3 Sobel + relu + maxpool2x2) ejecutado sobre el frame de la SHM; el NPU del Exynos 2100 sólo acepta INT8, por eso todo el grafo es cuANT8_ASYMM.".into(),
                measured: "Implementado en rust-core/src/npu_nnapi.rs (verificado por contrato de ABI; requiere hardware Exynos para la ejecución real)".into(),
            },
            AccelRule {
                workload: "vision-heavy-detector".into(),
                device: "none".into(),
                implemented: false,
                reason: "YOLO/SSD/OCR/CLIP INT8 por NNAPI requeriría un modelo cuantizado por tensor y NMS en CPU. No hay modelo en el repo: no se promete.".into(),
                measured: String::new(),
            },
            AccelRule {
                workload: "llm-on-npu".into(),
                device: "none".into(),
                implemented: false,
                reason: "Imposible con esta API: NNAPI no define operaciones de atención ni formas dinámicas, y llama.cpp no incluye backend NPU para Exynos. El precedente real (RK3588 con MUL_MAT nativo) midió decode MÁS LENTO en NPU que en CPU.".into(),
                measured: "rk-llama.cpp RK3588: NPU 5,3 tok/s vs CPU 7,4 tok/s en decode".into(),
            },
            AccelRule {
                workload: "gpu-vulkan-for-llm".into(),
                device: "none".into(),
                implemented: false,
                reason: "En Mali el backend Vulkan es más lento que la CPU (8x-22x medidos en otros SoCs Mali; 2x en el propio S21). No se habilita para el LLM.".into(),
                measured: "llama.cpp#22682 (Mali): CPU 35,9/25,7 tok/s vs Vulkan 3,3/5,7 tok/s; rk-llama.cpp (Mali-G610) 22x más lento".into(),
            },
        ],
        llama_build_flags: vec![
            "-DGGML_CPU_ARM_ARCH=armv8.2-a+dotprod+i8mm".into(),
            "-DGGML_CPU_KLEIDIAI=ON".into(),
            "-DGGML_OPENMP=OFF".into(),
        ],
        llama_server_flags: vec![
            "-t 4".into(),
            "--cache-type-k q8_0".into(),
            "--cache-type-v q8_0".into(),
            "--cont-batching".into(),
            "--flash-attn".into(),
        ],
        llm_cores: vec![4, 5, 6, 7],
        llm_threads: 4,
        vision_cores: vec![0, 1, 2, 3],
        evidence_version: "v13.2-2026-11".into(),
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn ninguna_regla_promete_npu_para_llm() {
        let p = policy();
        let llm_npu = p
            .rules
            .iter()
            .find(|r| r.workload == "llm-on-npu")
            .expect("debe existir la regla llm-on-npu");
        assert_eq!(llm_npu.device, "none");
        assert!(!llm_npu.implemented);
    }

    #[test]
    fn vision_si_declara_npu_implementado() {
        let p = policy();
        let v = p
            .rules
            .iter()
            .find(|r| r.workload == "vision-edge-detection")
            .unwrap();
        assert_eq!(v.device, "npu");
        assert!(v.implemented);
    }
}
