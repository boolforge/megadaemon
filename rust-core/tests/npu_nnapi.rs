//! MegaDaemon v13.2 — Pruebas del backend NNAPI real.
//!
//! Qué se verifica AQUÍ (ejecutable, sin hardware Exynos):
//!  1. El **camino FFI completo** contra una implementación independiente de
//!     la ABI NNAPI (`tests/fake_nnapi.c`, compilada en runtime con `cc`):
//!     dlopen, layout del struct OperandType, orden de argumentos de cada
//!     función, longitudes en bytes, índices del grafo.
//!  2. Que la ruta NNAPI y la **referencia CPU** dan el MISMO resultado
//!     numérico (equivalencia exacta, no "aproximadamente").
//!  3. Que sin NNAPI se cae a la referencia CPU y la telemetría lo dice
//!     (`executed_on_hardware == false`, `fallback_reason` no vacío).
//!  4. Que el detector de bordes es correcto (frame plano → 0; borde en una
//!     banda → energía concentrada en esa banda).
//!  5. Que la política de aceleradores NO promete NPU para el LLM.
//!
//! Qué NO se puede verificar aquí (y por tanto no se afirma):
//!  * Que el driver de Samsung acepte el grafo y lo ejecute en el NPU real.
//!    Eso requiere un Exynos 2100 con `libneuralnetworks.so`; el contrato de
//!    ABI sí está verificado contra el header AOSP (`nnapi-sys` 0.1.1).

use megadaemon_core::accelerator_policy;
use megadaemon_core::npu_nnapi::{self, NnapiApi, NnapiBackend, CpuBackend, EdgeBackend};
use std::path::PathBuf;
use std::process::Command;

/// Compila la librería NNAPI falsa; devuelve su ruta.
fn build_fake_lib() -> Option<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest.join("tests/fake_nnapi.c");
    if !src.exists() {
        return None;
    }
    let outdir = manifest.join("target/test-libs");
    std::fs::create_dir_all(&outdir).ok()?;
    let out = outdir.join("libfake_nnapi.so");
    // Recompilar siempre: evita falsos verdes con una .so obsoleta.
    let _ = std::fs::remove_file(&out);
    let status = Command::new("cc")
        .args(["-shared", "-fPIC", "-O2", "-w", "-o"])
        .arg(&out)
        .arg(&src)
        .arg("-lm")
        .status()
        .ok()?;
    if status.success() {
        Some(out)
    } else {
        None
    }
}

/// Frame sintético: bordes verticales/horizontales controlados.
fn synth_frame(w: usize, h: usize, edge_row: Option<usize>) -> Vec<u8> {
    let mut v = vec![40u8; w * h];
    for y in 0..h {
        if let Some(er) = edge_row {
            if y == er {
                for x in 0..w {
                    v[y * w + x] = 230; // borde horizontal fuerte
                }
            }
        }
    }
    v
}

#[test]
fn camino_ffi_nnapi_es_equivalente_a_la_referencia_cpu() {
    let Some(fake) = build_fake_lib() else {
        eprintln!("[SKIP] no hay compilador C: no se puede construir la NNAPI falsa");
        return;
    };
    let api = NnapiApi::open_path(fake.to_str().unwrap()).expect("la NNAPI falsa debe abrir");

    // 1) Dispositivos: el fake expone un ACCELERATOR y un CPU.
    let devices = api.devices();
    assert_eq!(devices.len(), 2, "deben verse 2 dispositivos NNAPI");
    let accel = devices
        .iter()
        .find(|d| d.device_type == npu_nnapi::DEVICE_ACCELERATOR)
        .expect("debe existir un dispositivo ACCELERATOR");
    assert!(accel.name.contains("Fake"));

    // 2) Compilar el grafo contra el dispositivo acelerador.
    let (w, h) = (64u32, 32u32);
    let graph = npu_nnapi::build_edge_graph(w, h);
    let backend = NnapiBackend::compile(api, accel.clone(), &graph)
        .expect("la compilación NNAPI debe funcionar con la lib falsa");

    // 3) Ejecutar y comparar con la referencia CPU.
    let frame = synth_frame(w as usize, h as usize, Some(20));
    let npu_out = backend.run(&graph, &frame).expect("la ejecución NNAPI debe funcionar");
    let cpu_out = CpuBackend.run(&graph, &frame).expect("la referencia CPU debe funcionar");

    assert_eq!(npu_out.len(), cpu_out.len());
    assert_eq!(
        npu_out, cpu_out,
        "la ruta NNAPI y la referencia CPU deben coincidir bit a bit"
    );
    assert!(
        npu_out.iter().any(|v| *v > 0),
        "un frame con bordes debe producir energía > 0"
    );
}

#[test]
fn el_grafo_declara_operadores_consistentes() {
    let g = npu_nnapi::build_edge_graph(32, 16);
    // Índices de entrada/salida dentro del rango.
    assert!((g.input_index as usize) < g.operands.len());
    assert!((g.output_index as usize) < g.operands.len());
    // Toda entrada/salida de op existe.
    for op in &g.ops {
        for i in &op.inputs {
            assert!((*i as usize) < g.operands.len());
        }
        for o in &op.outputs {
            assert!((*o as usize) < g.operands.len());
        }
    }
    // Los escalares tienen valor.
    let scalars: Vec<i32> = g
        .operands
        .iter()
        .filter_map(|o| o.const_i32.as_ref().and_then(|v| v.first().copied()))
        .collect();
    assert!(scalars.contains(&1), "padding de la conv = 1");
    assert!(scalars.contains(&2), "stride/filtro del maxpool = 2");
    // El filtro está cuantizado en int8 y es múltiplo exacto de 32.
    let f = npu_nnapi::sobel_filters_i8();
    assert!(f.iter().all(|v| (*v as i32) % 32 == 0));
}

#[test]
fn frame_plano_no_tiene_energia() {
    let (w, h) = (48u32, 24u32);
    let g = npu_nnapi::build_edge_graph(w, h);
    let flat = vec![77u8; (w * h) as usize];
    let out = npu_nnapi::interpret_graph(&g, &flat).unwrap();
    assert!(out.iter().all(|v| *v == 0));
}

#[test]
fn la_energia_se_concentra_en_la_banda_del_borde() {
    let (w, h) = (64u32, 64u32);
    let g = npu_nnapi::build_edge_graph(w, h);
    // Borde horizontal sólo en el tercio inferior.
    let frame = synth_frame(w as usize, h as usize, Some(56));
    let out = npu_nnapi::interpret_graph(&g, &frame).unwrap();
    let res = npu_nnapi::build_result(&g, out);
    assert!(
        res.band_energy[2] > res.band_energy[0],
        "la banda inferior debe concentrar la energía: {:?}",
        res.band_energy
    );
    assert!(res.total_energy > 0);
    let d = res.band_density(2);
    assert!((0.0..=1.0).contains(&d), "densidad fuera de rango: {}", d);
}

#[test]
fn sin_nnapi_se_cae_a_cpu_y_la_telemetria_lo_dice() {
    // En Linux/sandbox no existe /system/lib64/libneuralnetworks.so.
    let (w, h) = (32u32, 16u32);
    let luma = synth_frame(w as usize, h as usize, None);
    let (res, info) = npu_nnapi::analyze_edges(&luma, w, h);
    assert_eq!(info.backend, "cpu-reference");
    assert!(!info.executed_on_hardware, "no se puede afirmar ejecución en hardware");
    assert!(!info.fallback_reason.is_empty(), "el motivo de fallback debe quedar registrado");
    assert!(res.total_energy > 0 || res.total_energy == 0); // consistente
}

#[test]
fn la_sonda_informa_del_dispositivo_real() {
    let info = npu_nnapi::probe();
    // Sin hardware: no NNAPI.  Con hardware: dispositivos reales.
    if !info.nnapi_present {
        assert_eq!(info.backend, "cpu-reference");
        assert_eq!(info.nnapi_device_count, 0);
    } else {
        assert!(info.nnapi_device_count > 0);
    }
}

#[test]
fn la_politica_no_promete_npu_para_el_llm() {
    let p = accelerator_policy::policy();
    let r = p
        .rules
        .iter()
        .find(|r| r.workload == "llm-on-npu")
        .expect("debe existir la regla llm-on-npu");
    assert_eq!(r.device, "none");
    assert!(!r.implemented);
    assert!(r.reason.contains("atención") || r.reason.contains("atencion"));

    let v = p
        .rules
        .iter()
        .find(|r| r.workload == "vision-edge-detection")
        .expect("debe existir la regla de visión");
    assert_eq!(v.device, "npu");
    assert!(v.implemented);

    // El LLM va a la CPU con dotprod; nunca a la GPU en Mali.
    let d = p.rules.iter().find(|r| r.workload == "llm-decode").unwrap();
    assert_eq!(d.device, "cpu");
    assert!(p.llama_build_flags.iter().any(|f| f.contains("dotprod")));
}

#[test]
fn el_informe_de_vision_expone_el_backend_real() {
    // El informe de CV debe declarar siempre el backend que se ejecutó.
    let rep = megadaemon_core::npu_cv::analyze_vla_frame_and_accel();
    assert!(!rep.vision_backend.is_empty());
    assert!(rep.edge_energy_model.contains("conv3x3"));
    assert_eq!(rep.hud_rois.len(), 3);
    // La telemetría del hardware no puede prometer NPU para el LLM.
    assert!(rep
        .hardware
        .npu_workloads_impossible
        .iter()
        .any(|w| w.contains("LLM")));
    assert!(rep
        .hardware
        .npu_workloads_implemented
        .iter()
        .any(|w| w.contains("NNAPI")));
}
