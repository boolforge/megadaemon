use crate::accelerator_policy;
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_int;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NnapiDeviceInfo {
    pub index: u32,
    pub name: String,
    pub version: String,
    pub device_type: i32,
    pub type_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdenLibraryProbe {
    pub path: String,
    pub exists: bool,
    pub size_bytes: u64,
    pub dlopen_ok: bool,
    pub resolved_symbols: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExynosNpuTelemetry {
    pub soc: String,
    pub vs4l_device_node: String,
    pub vs4l_present: bool,
    pub kleidiai_dotprod_active: bool,
    pub ggml_backends: Vec<String>,
    pub nnapi_device_count: u32,
    pub nnapi_devices: Vec<NnapiDeviceInfo>,
    pub eden_runtime: EdenLibraryProbe,
    pub eden_ud_npu: EdenLibraryProbe,
    pub cpu_freqs_mhz: Vec<u32>,
    pub performance_cluster_pinned: bool,
    pub battery_temp_c: f32,
    pub cpu_temp_c: f32,
    pub throttle_state: String,
    pub throttle_delay_sec: f32,
    pub hybrid_routing_policy: String,
    /// ¿Hay un CAMINO REAL de offload al NPU?  (v13.1: honestidad)
    /// En v13.0 la "hybrid routing policy" era un STRING que prometía
    /// "Prefill -> Exynos NPU (EDEN/NNAPI)" pero NINGÚN módulo del repo
    /// invocaba EDEN/VNNI/NNAPI: el NPU era sólo telemetría.  Este flag lo
    /// dice explícitamente para que la UI no prometa lo que no hay.
    pub npu_offload_implemented: bool,
    pub npu_offload_note: String,
    /// Backend real que ejecutó el análisis de visión (v13.2).
    pub npu_vision_backend: String,
    /// Política REAL de aceleradores (JSON embebido) — sustituye al string
    /// de la "hybrid routing policy" de la v13.0, que no correspondía a ningún
    /// código ejecutado.
    pub accelerator_policy: accelerator_policy::AcceleratorPolicy,
}

fn probe_elf_symbols(lib_path: &str, symbols: &[&str]) -> EdenLibraryProbe {
    let p = Path::new(lib_path);
    let exists = p.exists();
    let size_bytes = fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let mut dlopen_ok = false;
    let mut resolved = Vec::new();

    if exists {
        if let Ok(c_path) = CString::new(lib_path) {
            unsafe {
                let handle = libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
                if !handle.is_null() {
                    dlopen_ok = true;
                    for &sym in symbols {
                        if let Ok(c_sym) = CString::new(sym) {
                            let ptr = libc::dlsym(handle, c_sym.as_ptr());
                            if !ptr.is_null() {
                                resolved.push(sym.to_string());
                            }
                        }
                    }
                    libc::dlclose(handle);
                } else {
                    // v13.2 (HW-10): antes se listaban los símbolos como
                    // "resueltos (ELF-verified)" aunque la librería NO se
                    // pudo cargar.  Ahora se declara el hecho sin fingir.
                    resolved.push(format!(
                        "dlopen fallo: {} simbolos presentes en el ELF pero NO cargados",
                        symbols.len()
                    ));
                }
            }
        }
    }

    EdenLibraryProbe {
        path: lib_path.to_string(),
        exists,
        size_bytes,
        dlopen_ok,
        resolved_symbols: resolved,
    }
}

fn check_kleidiai_dotprod() -> (bool, Vec<String>) {
    let mut backends = Vec::new();
    let mut dotprod = false;
    let base_dir = "/data/user/0/com.agy/files/usr/lib";

    //
    // v13.2 (HW-10): antes se anunciaba el backend si el FICHERO existía y
    // se declaraba dotprod=true aunque el dlopen fallara o el símbolo no
    // estuviera.  Ahora un backend sólo se anuncia si la librería se ha
    // CARGADO de verdad, y dotprod sólo se declara si el símbolo lo confirma.
    for (file, label) in &[
        ("libggml-cpu.so", "ggml-cpu (ARMv8.2 KleidiAI SDOT + FP16)"),
        ("libggml-vulkan.so", "ggml-vulkan (Mali-G78 MP14 Vulkan Compute)"),
        ("libggml-opencl.so", "ggml-opencl (Mali-G78 MP14 OpenCL 3.0)"),
    ] {
        let full = format!("{}/{}", base_dir, file);
        if !Path::new(&full).exists() {
            continue;
        }
        if let Ok(c_path) = CString::new(full.clone()) {
            unsafe {
                let h = libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
                if !h.is_null() {
                    backends.push(label.to_string());
                    if *file == "libggml-cpu.so" {
                        let sym = libc::dlsym(
                            h,
                            CStr::from_bytes_with_nul_unchecked(b"ggml_cpu_has_dotprod\0").as_ptr(),
                        );
                        if !sym.is_null() {
                            type HasDotprodFn = unsafe extern "C" fn() -> c_int;
                            let f: HasDotprodFn = std::mem::transmute(sym);
                            dotprod = f() != 0;
                        }
                    }
                    libc::dlclose(h);
                }
            }
        }
    }

    if !dotprod {
        if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
            if cpuinfo.contains("asimddp") {
                dotprod = true;
            }
        }
    }

    (dotprod, backends)
}

pub fn read_cpu_and_thermal() -> (Vec<u32>, f32, f32, String, f32) {
    // BUG v13.0: `freqs.push` sólo metía los núcleos que se podían leer, así
    // que si el core0 estaba offline (o no tenía cpufreq) el vector se
    // desplazaba y la UI atribtaba la frecuencia de un core a otro.  Ahora
    // el índice ES el número de core (0 si no se pudo leer).
    let ncpu = std::thread::available_parallelism()
        .map(|n| n.get().min(16))
        .unwrap_or(8);
    let mut freqs: Vec<u32> = Vec::with_capacity(ncpu);
    for core in 0..ncpu {
        let mut mhz = 0u32;
        for p in [
            format!("/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq", core),
            format!("/sys/devices/system/cpu/cpu{}/cpufreq/cpuinfo_max_freq", core),
        ] {
            if let Ok(s) = fs::read_to_string(&p) {
                if let Ok(khz) = s.trim().parse::<u32>() {
                    mhz = khz / 1000;
                    break;
                }
            }
        }
        freqs.push(mhz);
    }

    // v13.2 (HW-16): sin sensor NO se inventa una temperatura.  NaN se
    // serializa como `null` en JSON y las comparaciones con NaN dan false
    // (la politica termica no se dispara por un valor inventado).
    let mut bat_temp_c: f32 = f32::NAN;
    for bp in &[
        "/sys/class/power_supply/battery/temp",
        "/sys/class/power_supply/battery/batt_temp",
    ] {
        if let Ok(s) = fs::read_to_string(bp) {
            if let Ok(v) = s.trim().parse::<f32>() {
                bat_temp_c = if v > 100.0 { v / 10.0 } else { v };
                break;
            }
        }
    }

    // BUG v13.0: se tomaba el MAX de las 8 primeras zonas térmicas sin mirar
    // su tipo: en Android eso incluye el sensor de piel, el cargador o el
    // modem, y un "max" así puede declarar THROTTLE por el móvil que está en
    // la mesa.  Ahora se priorizan las zonas de SoC/GPU/CPU.
    let mut cpu_temp_c: f32 = f32::NAN;
    let mut best_is_soc = false;
    let zones = fs::read_dir("/sys/class/thermal").map(|it| it.flatten().count()).unwrap_or(16);
    for tz in 0..zones {
        let base = format!("/sys/class/thermal/thermal_zone{}", tz);
        let p = format!("{}/temp", base);
        let ztype = fs::read_to_string(format!("{}/type", base))
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let is_soc = ztype.contains("soc")
            || ztype.contains("cpu")
            || ztype.contains("gpu")
            || ztype.contains("cluster")
            || ztype.contains("big")
            || ztype.contains("little");
        if let Ok(s) = fs::read_to_string(&p) {
            if let Ok(v) = s.trim().parse::<f32>() {
                let c = if v > 1000.0 { v / 1000.0 } else { v };
                if c <= 20.0 || c >= 115.0 {
                    continue;
                }
                if cpu_temp_c.is_nan() {
                    cpu_temp_c = c;
                    best_is_soc = is_soc;
                } else if (is_soc && !best_is_soc) || (is_soc == best_is_soc && c > cpu_temp_c) {
                    cpu_temp_c = c;
                    best_is_soc = is_soc;
                }
            }
        }
    }

    let (state, delay) = if bat_temp_c >= 41.0 || cpu_temp_c >= 72.0 {
        ("HOT_THROTTLED".to_string(), 12.0)
    } else if bat_temp_c >= 38.0 || cpu_temp_c >= 62.0 {
        ("WARM_BALANCED".to_string(), 5.0)
    } else {
        ("COOL_TURBO".to_string(), 1.5)
    };

    (freqs, bat_temp_c, cpu_temp_c, state, delay)
}

/// Pins current process and any running `llama-server` threads to Exynos 2100 performance cluster (Cores 4-7: 3x Cortex-A78 + 1x Cortex-X1)
pub fn pin_performance_cluster() -> bool {
    let mut pinned = false;
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        for core in 4..=7 {
            libc::CPU_SET(core, &mut set);
        }
        if libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0 {
            pinned = true;
        }
    }

    // Also pin running llama-server processes via taskset -a -p f0 <pid>
    if let Ok(out) = Command::new("pgrep").arg("-f").arg("llama-server").output() {
        let s = String::from_utf8_lossy(&out.stdout);
        for line in s.lines() {
            if let Ok(pid) = line.trim().parse::<i32>() {
                unsafe {
                    let mut set: libc::cpu_set_t = std::mem::zeroed();
                    for core in 4..=7 {
                        libc::CPU_SET(core, &mut set);
                    }
                    if libc::sched_setaffinity(pid, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0 {
                        pinned = true;
                    }
                }
            }
        }
    }

    pinned
}

static PROBE_CACHE: OnceLock<Mutex<(std::time::Instant, ExynosNpuTelemetry)>> = OnceLock::new();

/// TTL del sondeo cacheado: la UI pregunta cada 20 s, pero varios endpoints
/// hitcan la misma telemetría; sin cache, cada petición releía /sys entero.
const PROBE_TTL_SECS: u64 = 5;

/// Sondeo de hardware con cache corto.
pub fn probe_exynos_npu_cached() -> ExynosNpuTelemetry {
    let cache = PROBE_CACHE.get_or_init(|| Mutex::new((std::time::Instant::now() - std::time::Duration::from_secs(PROBE_TTL_SECS), probe_exynos_npu())));
    let mut guard = cache.lock().unwrap();
    if guard.0.elapsed().as_secs() < PROBE_TTL_SECS {
        return guard.1.clone();
    }
    let fresh = probe_exynos_npu();
    *guard = (std::time::Instant::now(), fresh.clone());
    fresh
}

pub fn probe_exynos_npu() -> ExynosNpuTelemetry {
    // v13.2: la v13.0/1 hacía esto:
    //     su -c "MEGADAEMON_NPU_RAW=1 <exe> npu-probe"
    // es decir, CONVOCABA OTRO PROCESO (con `su`, que puede pedir contrasena y
    // COLGARSE sin limite) en CADA sondeo, y la UI sondea cada 20 s.  Un
    // `su` interactivo sin timeout puede tumbar el cockpit entero.
    //
    // Ahora: sondeo directo (sin root) + cacheo corto.  Lo que necesite root
    // se declara como no disponible en vez de colgarse.
    let vs4l_node = "/dev/vertex10";
    let vs4l_present = Path::new(vs4l_node).exists();
    let (kleidiai_dotprod_active, ggml_backends) = check_kleidiai_dotprod();
    let nnapi_probe = crate::npu_nnapi::probe();
    let nnapi_device_count = nnapi_probe.nnapi_device_count;
    let nnapi_devices: Vec<NnapiDeviceInfo> = nnapi_probe
        .nnapi_devices
        .iter()
        .map(|d| NnapiDeviceInfo {
            index: d.index,
            name: d.name.clone(),
            version: d.version.clone(),
            device_type: d.device_type,
            type_label: d.type_label.clone(),
        })
        .collect();

    let eden_runtime = probe_elf_symbols(
        "/vendor/lib64/libeden_wrapper.so",
        &[
            "Initialize",
            "GetEdenVersion",
            "OpenModelFromMemory",
            "AllocateInputBuffers",
            "AllocateOutputBuffers",
            "ExecuteModel",
            "FreeBuffers",
            "CloseModel",
            "Shutdown",
        ],
    );

    let eden_ud_npu = probe_elf_symbols(
        "/vendor/lib64/libeden_ud_npu.so",
        &[
            "_ZN4eden2ud13NpuUserDriver10GetDrvInfoEP9_drv_info",
            "_ZN4eden2ud13NpuUserDriver20GetDeviceHealthStateEP21EdenDeviceHealthState",
            "apply_dynamic_boost_off",
            "eden_mem_free",
        ],
    );

    let (cpu_freqs_mhz, battery_temp_c, cpu_temp_c, throttle_state, throttle_delay_sec) =
        read_cpu_and_thermal();
    let performance_cluster_pinned = pin_performance_cluster();

    ExynosNpuTelemetry {
        soc: "Samsung Exynos 2100 (S5E9840) — 1x Cortex-X1 + 3x Cortex-A78 + 4x Cortex-A55 + 26-TOPS Triple-Core NPU".to_string(),
        vs4l_device_node: vs4l_node.to_string(),
        vs4l_present,
        kleidiai_dotprod_active,
        ggml_backends,
        nnapi_device_count,
        nnapi_devices,
        eden_runtime,
        eden_ud_npu,
        cpu_freqs_mhz,
        performance_cluster_pinned,
        battery_temp_c,
        cpu_temp_c,
        throttle_state,
        throttle_delay_sec,
        hybrid_routing_policy: "v13.2: la politica hibrida es REAL y esta en accelerator_policy::policy(). Resumen: vision (bordes INT8) -> NPU via NNAPI; LLM (decode+prefill) -> CPU con dotprod+i8mm+KleidiAI en cores 4-7; Vulkan/Mali NO para el LLM (medido 2x mas lento en decode en el Galaxy S21); el NPU NO puede ejecutar el LLM (NNAPI no tiene ops de atencion ni formas dinamicas; llama.cpp no tiene backend Exynos).".to_string(),
        npu_offload_implemented: true,
        npu_offload_note: "v13.2: S\u{FA} hay offload real. La tuberia de vision INT8 (conv3x3 Sobel + relu + maxpool2x2, forma fija) se compila y ejecuta via NNAPI (libneuralnetworks.so, ABI verificada contra AOSP) sobre el dispositivo ACCELERATOR (NPU del Exynos 2100) cuando esta disponible; si el driver rechaza el modelo se cae a la referencia CPU con la MISMA aritmetica entera y la telemetria lo dice (vision_backend / vision_fallback_reason). El LLM NO va al NPU: es fisicamente imposible con esta API.".to_string(),
        npu_vision_backend: nnapi_probe.backend.clone(),
        accelerator_policy: accelerator_policy::policy(),
    }
}
