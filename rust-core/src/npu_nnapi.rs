//! MegaDaemon v13.2 — Backend NNAPI REAL (Android Neural Networks API, INT8).
//!
//! # Por qué este módulo existe
//!
//! En v13.0 el "NPU" de MegaDaemon era TELEMETRÍA: un string que prometía
//! "Prefill -> Exynos NPU (EDEN/NNAPI)" mientras ningún módulo invocaba
//! NNAPI, EDEN ni VNNI.  Este módulo implementa el primer camino REAL de
//! offload a acelerador del repo:
//!
//! * Se habla **NNAPI** (la API pública de Android, `/system/lib64/libneuralnetworks.so`,
//!   ABI verificada contra el header AOSP `neural_networks.h`).
//! * Se ejecuta un grafo **convolucional INT8 de forma fija** (CONV_2D 3x3 con
//!   filtros Sobel + RELU + MAX_POOL_2D 2x2) sobre el frame de la SHM.
//! * Si no hay NNAPI, o el driver rechaza el modelo, se cae a una
//!   **referencia en CPU con la MISMA aritmética entera**, y la telemetría
//!   dice exactamente qué backend se ejecutó.
//!
//! # Qué NO es verdad (y por tanto no se promete)
//!
//! * El NPU del Exynos 2100 es **INT8-only** (fuente: ETH Zürich, "Deep
//!   Learning on Mobile Devices", AIM'21): un grafo float32 NUNCA tocaría el
//!   NPU, caería en la GPU o en la CPU.  Por eso TODO este módulo es INT8.
//! * El NPU **no ejecuta el LLM**.  NNAPI no tiene ops de atención ni
//!   soporta formas dinámicas; llama.cpp no tiene backend Exynos; y el
//!   precedente real (RK3588, NPU con MUL_MAT nativo) demuestra que el
//!   decode de un LLM en NPU es MÁS LENTO que en CPU por el ping-pong de
//!   tensores.  Ver `docs/NPU_REAL.md`.
//! * En este sandbox no hay hardware Exynos ni `libneuralnetworks.so`: las
//!   pruebas que se ejecutan aquí validan el GRAFO, la ARITMÉTICA, el
//!   FALLBACK y el TELEMETRÍA.  La ejecución en NPU está verificada por
//!   contrato de ABI, no ejecutada.
//!
//! # Aritmética (exacta, sin flotantes en el camino crítico)
//!
//! ```text
//! q_x   = luma            (u8,  s_in = 1.0,   zp = 0)
//! q_w   = round(w/s_f)    (i8,  s_f = 1/32,  zp = 0)   -> Sobel exacto: ±32, ±64
//! acc   = sum(q_x * q_w) + bias            (i32, escala s_in*s_f = 1/32)
//! q_out = clamp(round(acc * (s_in*s_f) / s_out) + zp_out, -128, 127)
//!         (s_out = 8.0 -> |q_out| <= 127 con luma en [0,255])
//! relu  = clamp(q_out, 0, 127)
//! pool  = max 2x2, stride 2, VALID
//! ```
//!
//! El driver calcula la requantización con redondeo "nearest"; nuestra
//! referencia usa round-half-away-from-zero.  La diferencia posible es de
//! ±1 LSB (8 unidades de luma) en un pixel; el test delega esa tolerancia
//! explícitamente (ver `tests/npu_nnapi.rs`).

use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::ptr;

// ─────────────────────────────────────────────────────────────────────────────
// Constantes NNAPI (valores verificados contra AOSP neural_networks.h)
// ─────────────────────────────────────────────────────────────────────────────

/// OperandCode::TENSOR_QUANT8_ASYMM
pub const TENSOR_QUANT8_ASYMM: i32 = 5;
/// OperandCode::TENSOR_INT32
pub const TENSOR_INT32: i32 = 4;
/// OperationType::CONV_2D
pub const OP_CONV_2D: i32 = 3;
/// OperationType::MAX_POOL_2D
pub const OP_MAX_POOL_2D: i32 = 17;
/// OperationType::RELU
pub const OP_RELU: i32 = 19;
/// FuseCode::NONE
pub const FUSED_NONE: i32 = 0;
/// PaddingCode::SAME
pub const PADDING_SAME: i32 = 1;
/// PaddingCode::VALID
pub const PADDING_VALID: i32 = 2;
/// PreferenceCode::LOW_POWER (el NPU sólo aparece con LOW_POWER en Exynos)
pub const PREFER_LOW_POWER: i32 = 0;
/// PreferenceCode::FAST_SINGLE_ANSWER
pub const PREFER_FAST_SINGLE_ANSWER: i32 = 1;
/// DeviceTypeCode::CPU
pub const DEVICE_CPU: i32 = 2;
/// DeviceTypeCode::GPU
pub const DEVICE_GPU: i32 = 3;
/// DeviceTypeCode::ACCELERATOR (el NPU de Exynos)
pub const DEVICE_ACCELERATOR: i32 = 4;

/// Ruta de la librería NDK de NNAPI (API pública, existe desde API 27).
pub const NNAPI_LIB_PATH: &str = "/system/lib64/libneuralnetworks.so";

// ─────────────────────────────────────────────────────────────────────────────
// FFI
// ─────────────────────────────────────────────────────────────────────────────

/// `ANeuralNetworksOperandType` — layout AOSP:
/// `int32 type; uint32 dimensionCount; const uint32* dimensions; float scale; int32 zeroPoint;`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OperandType {
    pub type_: i32,
    pub dimension_count: u32,
    pub dimensions: *const u32,
    pub scale: f32,
    pub zero_point: i32,
}

impl OperandType {
    pub fn scalar(type_: i32) -> Self {
        OperandType {
            type_,
            dimension_count: 0,
            dimensions: ptr::null(),
            scale: 0.0,
            zero_point: 0,
        }
    }

    /// Tensor con forma fija.  El llamante debe mantener vivo el `Vec<u32>`
    /// de dimensiones mientras dura la llamada NNAPI (el driver las copia,
    /// pero no podemos dejar un puntero colgante).
    pub fn tensor(type_: i32, dims: &[u32], scale: f32, zero_point: i32) -> Self {
        OperandType {
            type_,
            dimension_count: dims.len() as u32,
            dimensions: dims.as_ptr(),
            scale,
            zero_point,
        }
    }
}

// Punteros a función (firmas exactas del header AOSP).
type FnGetDeviceCount = unsafe extern "C" fn(*mut c_uint) -> c_int;
type FnGetDevice = unsafe extern "C" fn(c_uint, *mut *mut c_void) -> c_int;
type FnDeviceGetName = unsafe extern "C" fn(*mut c_void, *mut *const c_char) -> c_int;
type FnDeviceGetVersion = unsafe extern "C" fn(*mut c_void, *mut *const c_char) -> c_int;
type FnDeviceGetType = unsafe extern "C" fn(*mut c_void, *mut i32) -> c_int;
type FnModelCreate = unsafe extern "C" fn(*mut *mut c_void) -> c_int;
type FnModelAddOperand = unsafe extern "C" fn(*mut c_void, *const OperandType) -> c_int;
type FnModelSetOperandValue =
    unsafe extern "C" fn(*mut c_void, c_uint, *const c_void, usize) -> c_int;
type FnModelAddOperation =
    unsafe extern "C" fn(*mut c_void, c_int, c_uint, *const c_uint, c_uint, *const c_uint) -> c_int;
type FnModelFinish = unsafe extern "C" fn(*mut c_void) -> c_int;
type FnModelFree = unsafe extern "C" fn(*mut c_void);
type FnCompilationCreate = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> c_int;
type FnCompilationCreateForDevices =
    unsafe extern "C" fn(*mut c_void, *const *mut c_void, usize, *mut *mut c_void) -> c_int;
type FnCompilationSetPreference = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type FnCompilationFinish = unsafe extern "C" fn(*mut c_void) -> c_int;
type FnCompilationFree = unsafe extern "C" fn(*mut c_void);
type FnExecutionCreate = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> c_int;
type FnExecutionSetInput =
    unsafe extern "C" fn(*mut c_void, c_uint, *const OperandType, *const c_void, usize) -> c_int;
type FnExecutionSetOutput =
    unsafe extern "C" fn(*mut c_void, c_uint, *const OperandType, *mut c_void, usize) -> c_int;
type FnExecutionCompute = unsafe extern "C" fn(*mut c_void) -> c_int;
type FnExecutionFree = unsafe extern "C" fn(*mut c_void);

/// Envoltorio sobre la librería NNAPI cargada con `dlopen`.
///
/// Se carga en RUNTIME a propósito: el crate debe compilar en Linux (CI,
/// sandbox, desktop) sin `libneuralnetworks.so` y sin dependencia de
/// link-time.  En el móvil la ruta existe y se resuelve sin problema.
pub struct NnapiApi {
    handle: *mut c_void,
    get_device_count: FnGetDeviceCount,
    get_device: FnGetDevice,
    device_get_name: FnDeviceGetName,
    device_get_version: FnDeviceGetVersion,
    device_get_type: FnDeviceGetType,
    model_create: FnModelCreate,
    model_add_operand: FnModelAddOperand,
    model_set_operand_value: FnModelSetOperandValue,
    model_add_operation: FnModelAddOperation,
    model_finish: FnModelFinish,
    model_free: FnModelFree,
    compilation_create: FnCompilationCreate,
    compilation_create_for_devices: FnCompilationCreateForDevices,
    compilation_set_preference: FnCompilationSetPreference,
    compilation_finish: FnCompilationFinish,
    compilation_free: FnCompilationFree,
    execution_create: FnExecutionCreate,
    execution_set_input: FnExecutionSetInput,
    execution_set_output: FnExecutionSetOutput,
    execution_compute: FnExecutionCompute,
    execution_free: FnExecutionFree,
}

fn dlsym_opt(handle: *mut c_void, name: &[u8]) -> Option<*mut c_void> {
    let sym = unsafe { libc::dlsym(handle, CStr::from_bytes_with_nul_unchecked(name).as_ptr()) };
    if sym.is_null() {
        None
    } else {
        Some(sym)
    }
}

macro_rules! need_sym {
    ($handle:expr, $name:expr) => {
        match dlsym_opt($handle, $name) {
            Some(p) => unsafe { std::mem::transmute(p) },
            None => return None,
        }
    };
}

impl NnapiApi {
    /// Carga `libneuralnetworks.so` y resuelve los símbolos requeridos.
    /// Devuelve `None` si la librería no existe (p. ej. en Linux) o si falta
    /// algún símbolo (API más nueva de lo esperado).
    pub fn open() -> Option<NnapiApi> {
        Self::open_path(NNAPI_LIB_PATH)
    }

    pub fn open_path(path: &str) -> Option<NnapiApi> {
        let c_path = CString::new(path).ok()?;
        let handle = unsafe { libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        if handle.is_null() {
            return None;
        }

        let get_device_count: FnGetDeviceCount =
            need_sym!(handle, b"ANeuralNetworks_getDeviceCount\0");
        let get_device: FnGetDevice = need_sym!(handle, b"ANeuralNetworks_getDevice\0");
        let device_get_name: FnDeviceGetName = need_sym!(handle, b"ANeuralNetworksDevice_getName\0");
        let device_get_version: FnDeviceGetVersion =
            need_sym!(handle, b"ANeuralNetworksDevice_getVersion\0");
        let device_get_type: FnDeviceGetType = need_sym!(handle, b"ANeuralNetworksDevice_getType\0");
        let model_create: FnModelCreate = need_sym!(handle, b"ANeuralNetworksModel_create\0");
        let model_add_operand: FnModelAddOperand =
            need_sym!(handle, b"ANeuralNetworksModel_addOperand\0");
        let model_set_operand_value: FnModelSetOperandValue =
            need_sym!(handle, b"ANeuralNetworksModel_setOperandValue\0");
        let model_add_operation: FnModelAddOperation =
            need_sym!(handle, b"ANeuralNetworksModel_addOperation\0");
        let model_finish: FnModelFinish = need_sym!(handle, b"ANeuralNetworksModel_finish\0");
        let model_free: FnModelFree = need_sym!(handle, b"ANeuralNetworksModel_free\0");
        let compilation_create: FnCompilationCreate =
            need_sym!(handle, b"ANeuralNetworksCompilation_create\0");
        let compilation_create_for_devices: FnCompilationCreateForDevices =
            need_sym!(handle, b"ANeuralNetworksCompilation_createForDevices\0");
        let compilation_set_preference: FnCompilationSetPreference =
            need_sym!(handle, b"ANeuralNetworksCompilation_setPreference\0");
        let compilation_finish: FnCompilationFinish =
            need_sym!(handle, b"ANeuralNetworksCompilation_finish\0");
        let compilation_free: FnCompilationFree =
            need_sym!(handle, b"ANeuralNetworksCompilation_free\0");
        let execution_create: FnExecutionCreate = need_sym!(handle, b"ANeuralNetworksExecution_create\0");
        let execution_set_input: FnExecutionSetInput =
            need_sym!(handle, b"ANeuralNetworksExecution_setInput\0");
        let execution_set_output: FnExecutionSetOutput =
            need_sym!(handle, b"ANeuralNetworksExecution_setOutput\0");
        let execution_compute: FnExecutionCompute =
            need_sym!(handle, b"ANeuralNetworksExecution_compute\0");
        let execution_free: FnExecutionFree = need_sym!(handle, b"ANeuralNetworksExecution_free\0");

        Some(NnapiApi {
            handle,
            get_device_count,
            get_device,
            device_get_name,
            device_get_version,
            device_get_type,
            model_create,
            model_add_operand,
            model_set_operand_value,
            model_add_operation,
            model_finish,
            model_free,
            compilation_create,
            compilation_create_for_devices,
            compilation_set_preference,
            compilation_finish,
            compilation_free,
            execution_create,
            execution_set_input,
            execution_set_output,
            execution_compute,
            execution_free,
        })
    }

    /// Lista los dispositivos NNAPI con su tipo (CPU / GPU / ACCELERATOR).
    pub fn devices(&self) -> Vec<NnapiDevice> {
        self.device_handles().into_iter().map(|(_, d)| d).collect()
    }

    /// Dispositivos con su handle crudo (necesario para `createForDevices`).
    pub fn device_handles(&self) -> Vec<(*mut c_void, NnapiDevice)> {
        let mut out = Vec::new();
        let mut count: c_uint = 0;
        unsafe {
            if (self.get_device_count)(&mut count) != 0 {
                return out;
            }
            for idx in 0..count {
                let mut dev: *mut c_void = ptr::null_mut();
                if (self.get_device)(idx, &mut dev) != 0 || dev.is_null() {
                    continue;
                }
                let mut name_ptr: *const c_char = ptr::null();
                let name = if (self.device_get_name)(dev, &mut name_ptr) == 0 && !name_ptr.is_null() {
                    CStr::from_ptr(name_ptr).to_string_lossy().into_owned()
                } else {
                    format!("nnapi-device-{}", idx)
                };
                let mut ver_ptr: *const c_char = ptr::null();
                let version = if (self.device_get_version)(dev, &mut ver_ptr) == 0 && !ver_ptr.is_null()
                {
                    CStr::from_ptr(ver_ptr).to_string_lossy().into_owned()
                } else {
                    "?".to_string()
                };
                let mut dev_type: i32 = 0;
                (self.device_get_type)(dev, &mut dev_type);
                out.push((
                    dev,
                    NnapiDevice {
                        index: idx,
                        name,
                        version,
                        device_type: dev_type,
                        type_label: device_type_label(dev_type).to_string(),
                    },
                ));
            }
        }
        out
    }
}

impl Drop for NnapiApi {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { libc::dlclose(self.handle) };
        }
    }
}

/// Dispositivo NNAPI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NnapiDevice {
    pub index: u32,
    pub name: String,
    pub version: String,
    pub device_type: i32,
    pub type_label: String,
}

pub fn device_type_label(t: i32) -> &'static str {
    match t {
        1 => "OTHER",
        DEVICE_CPU => "CPU",
        DEVICE_GPU => "GPU",
        DEVICE_ACCELERATOR => "ACCELERATOR (NPU)",
        _ => "UNKNOWN",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Descripción del grafo (datos, no código: así se puede testear y auditar)
// ─────────────────────────────────────────────────────────────────────────────

/// Un operando del grafo NNAPI.
#[derive(Debug, Clone)]
pub struct OperandDesc {
    pub type_: i32,
    pub dims: Vec<u32>,
    pub scale: f32,
    pub zero_point: i32,
    /// Valor estático (enteros) si el operando es constante.
    pub const_i32: Option<Vec<i32>>,
    /// Valor estático (bytes) si el operando es un tensor constante.
    pub const_bytes: Option<Vec<u8>>,
}

/// Una operación del grafo NNAPI.
#[derive(Debug, Clone)]
pub struct OpDesc {
    pub op: i32,
    pub inputs: Vec<u32>,
    pub outputs: Vec<u32>,
}

/// El grafo completo: conv 3x3 (Sobel X/Y) + relu + maxpool 2x2.
#[derive(Debug, Clone)]
pub struct GraphDesc {
    pub operands: Vec<OperandDesc>,
    pub ops: Vec<OpDesc>,
    pub input_index: u32,
    pub output_index: u32,
    pub out_h: u32,
    pub out_w: u32,
    pub in_scale: f32,
    pub filter_scale: f32,
    pub out_scale: f32,
}

/// Escala del filtro: 1/32 → los pesos Sobel (±1, ±2) son exactos en int8.
pub const FILTER_SCALE: f32 = 1.0 / 32.0;
/// Escala de salida: 8.0 → |q_out| <= 127 para luma en [0,255].
pub const OUT_SCALE: f32 = 8.0;

/// Pesos Sobel empaquetados: canal 0 = X, canal 1 = Y.
pub fn sobel_filters_i8() -> Vec<i8> {
    // Layout NHWC del filtro: [depth_out=2][fh=3][fw=3][depth_in=1]
    let gx: [i32; 9] = [-1, 0, 1, -2, 0, 2, -1, 0, 1];
    let gy: [i32; 9] = [-1, -2, -1, 0, 0, 0, 1, 2, 1];
    let mut out = Vec::with_capacity(18);
    for v in gx.iter() {
        out.push((v * 32) as i8); // w / (1/32) = w*32
    }
    for v in gy.iter() {
        out.push((v * 32) as i8);
    }
    out
}

/// Construye la descripción del grafo para una imagen de `w`x`h`.
///
/// Las formas son FIJAS (requisito de NNAPI: nada de formas dinámicas), por
/// lo que el modelo se compila una vez por resolución y se reutiliza.
pub fn build_edge_graph(w: u32, h: u32) -> GraphDesc {
    let mut ops: Vec<OperandDesc> = Vec::new();

    // 0: entrada (dinámica)
    ops.push(OperandDesc {
        type_: TENSOR_QUANT8_ASYMM,
        dims: vec![1, h, w, 1],
        scale: 1.0,
        zero_point: 0,
        const_i32: None,
        const_bytes: None,
    });
    // 1: pesos (constantes)
    ops.push(OperandDesc {
        type_: TENSOR_QUANT8_ASYMM,
        dims: vec![2, 3, 3, 1],
        scale: FILTER_SCALE,
        zero_point: 0,
        const_i32: None,
        const_bytes: Some(sobel_filters_i8().iter().map(|v| *v as u8).collect()),
    });
    // 2: bias (constante) — en el dominio del acumulador (escala s_in*s_f)
    ops.push(OperandDesc {
        type_: TENSOR_INT32,
        dims: vec![2],
        scale: FILTER_SCALE,
        zero_point: 0,
        const_i32: Some(vec![0, 0]),
        const_bytes: None,
    });

    // 3..: escalares
    let push_scalar = |ops: &mut Vec<OperandDesc>, v: i32| {
        ops.push(OperandDesc {
            type_: TENSOR_INT32,
            dims: vec![],
            scale: 0.0,
            zero_point: 0,
            const_i32: Some(vec![v]),
            const_bytes: None,
        });
    };
    // conv explícito: padL, padR, padT, padB, strideW, strideH, fused
    // Padding VALID (0): con zero-padding el marco del frame produce un
    // "borde" artificial que contamina la energía de la banda superior.
    for v in [0, 0, 0, 0, 1, 1, FUSED_NONE] {
        push_scalar(&mut ops, v);
    }
    // maxpool explícito: padL, padR, padT, padB, strideW, strideH, filtW, filtH, fused
    for v in [0, 0, 0, 0, 2, 2, 2, 2, FUSED_NONE] {
        push_scalar(&mut ops, v);
    }

    const CONV_OUT: u32 = 19;
    const RELU_OUT: u32 = 20;
    const POOL_OUT: u32 = 21;

    // 19: salida conv (cuantizada) — VALID: (h-2)x(w-2)
    let ch = h.saturating_sub(2).max(1);
    let cw = w.saturating_sub(2).max(1);
    ops.push(OperandDesc {
        type_: TENSOR_QUANT8_ASYMM,
        dims: vec![1, ch, cw, 2],
        scale: OUT_SCALE,
        zero_point: 0,
        const_i32: None,
        const_bytes: None,
    });
    // 20: salida relu
    ops.push(OperandDesc {
        type_: TENSOR_QUANT8_ASYMM,
        dims: vec![1, ch, cw, 2],
        scale: OUT_SCALE,
        zero_point: 0,
        const_i32: None,
        const_bytes: None,
    });
    // 21: salida pool
    let ph = ch.saturating_sub(2) / 2 + 1;
    let pw = cw.saturating_sub(2) / 2 + 1;
    ops.push(OperandDesc {
        type_: TENSOR_QUANT8_ASYMM,
        dims: vec![1, ph, pw, 2],
        scale: OUT_SCALE,
        zero_point: 0,
        const_i32: None,
        const_bytes: None,
    });

    let op_list = vec![
        // conv: [img, filter, bias, padL, padR, padT, padB, strideW, strideH, fused]
        OpDesc {
            op: OP_CONV_2D,
            inputs: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            outputs: vec![CONV_OUT],
        },
        OpDesc { op: OP_RELU, inputs: vec![CONV_OUT], outputs: vec![RELU_OUT] },
        // pool: [in, padL, padR, padT, padB, strideW, strideH, filtW, filtH, fused]
        OpDesc {
            op: OP_MAX_POOL_2D,
            inputs: vec![RELU_OUT, 10, 11, 12, 13, 14, 15, 16, 17, 18],
            outputs: vec![POOL_OUT],
        },
    ];

    GraphDesc {
        operands: ops,
        ops: op_list,
        input_index: 0,
        output_index: POOL_OUT,
        out_h: ph,
        out_w: pw,
        in_scale: 1.0,
        filter_scale: FILTER_SCALE,
        out_scale: OUT_SCALE,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Interpretador de referencia (CPU) — misma aritmética entera que el NPU
// ─────────────────────────────────────────────────────────────────────────────

/// Redondeo "nearest, ties away from zero" (equivale a la política de
/// requantización de TFLite/NNAPI en la práctica).
fn round_away(x: f32) -> i32 {
    if x >= 0.0 {
        (x + 0.5).floor() as i32
    } else {
        (x - 0.5).ceil() as i32
    }
}

/// Referencia en CPU del grafo NNAPI, calculada sobre la DESCRIPCIÓN del
/// grafo (no sobre código hardcodeado): si el grafo se corrompe, la
/// referencia y el test lo detectan.
pub fn interpret_graph(graph: &GraphDesc, input: &[u8]) -> Result<Vec<i8>, String> {
    let w = graph.operands[graph.input_index as usize].dims[2] as usize;
    let h = graph.operands[graph.input_index as usize].dims[1] as usize;
    if input.len() < w * h {
        return Err(format!("entrada corta: {} < {}", input.len(), w * h));
    }

    // Buffers por operando (i8 para tensores cuantizados, i32 escalares).
    let mut tensors: Vec<Option<Vec<i8>>> = vec![None; graph.operands.len()];
    let mut scalars: Vec<Option<i32>> = vec![None; graph.operands.len()];

    for (i, od) in graph.operands.iter().enumerate() {
        if let Some(v) = &od.const_i32 {
            scalars[i] = v.first().copied();
        }
    }

    // OJO: la entrada del grafo es TENSOR_QUANT8_ASYMM (u8, 0..255).  No se
    // puede guardar en un buffer i8 (230 -> -26) y ése fue un bug REAL
    // detectado por la prueba FFI contra la NNAPI falsa.

    for op in &graph.ops {
        match op.op {
            OP_CONV_2D => {
                let img = tensors[op.inputs[0] as usize].clone();
                // Si el operando de entrada es la entrada del grafo, se usa
                // el buffer u8 original (valores 0..255 sin signo).
                let graph_input = op.inputs[0] == graph.input_index;
                if !graph_input && img.is_none() {
                    return Err("conv: entrada no disponible".to_string());
                }
                let filt_bytes = graph.operands[op.inputs[1] as usize]
                    .const_bytes
                    .clone()
                    .ok_or("conv: filtro no constante")?;
                let bias = graph.operands[op.inputs[2] as usize]
                    .const_i32
                    .clone()
                    .unwrap_or_default();
                let pad_l = scalars[op.inputs[3] as usize].unwrap_or(0) as usize;
                let pad_r = scalars[op.inputs[4] as usize].unwrap_or(0) as usize;
                let pad_t = scalars[op.inputs[5] as usize].unwrap_or(0) as usize;
                let pad_b = scalars[op.inputs[6] as usize].unwrap_or(0) as usize;
                let sw = scalars[op.inputs[7] as usize].unwrap_or(1) as usize;
                let sh = scalars[op.inputs[8] as usize].unwrap_or(1) as usize;

                let od = &graph.operands[op.outputs[0] as usize];
                let (oh, ow, od_out) = (od.dims[1] as usize, od.dims[2] as usize, od.dims[3] as usize);
                let out_scale = od.scale;
                let in_scale = graph.in_scale;
                let filt_scale = graph.filter_scale;
                let eff = in_scale * filt_scale;

                let mut out = vec![0i8; oh * ow * od_out];
                for y in 0..oh {
                    for x in 0..ow {
                        for c in 0..od_out {
                            let mut acc: i32 = *bias.get(c).unwrap_or(&0);
                            for dy in 0..3usize {
                                for dx in 0..3usize {
                                    let iy = (y * sh + dy) as isize - pad_t as isize;
                                    let ix = (x * sw + dx) as isize - pad_l as isize;
                                    let qx = if iy < 0 || ix < 0 || iy >= h as isize || ix >= w as isize
                                    {
                                        0i32
                                    } else if graph_input {
                                        input[(iy as usize) * w + ix as usize] as i32
                                    } else {
                                        img.as_ref().unwrap()[(iy as usize) * w + ix as usize] as i32
                                    };
                                    let fw = filt_bytes[(c * 9 + dy * 3 + dx) as usize] as i8 as i32;
                                    acc += qx * fw;
                                }
                            }
                            // requantización: real = acc*eff ; q = real/out_scale
                            let real = acc as f32 * eff;
                            let mut q = round_away(real / out_scale);
                            q = q.clamp(-128, 127);
                            out[(y * ow + x) * od_out + c] = q as i8;
                        }
                    }
                }
                let _ = (pad_r, pad_b);
                tensors[op.outputs[0] as usize] = Some(out);
            }
            OP_RELU => {
                let src = tensors[op.inputs[0] as usize]
                    .clone()
                    .ok_or("relu: entrada no disponible")?;
                let out: Vec<i8> = src.into_iter().map(|v| v.max(0)).collect();
                tensors[op.outputs[0] as usize] = Some(out);
            }
            OP_MAX_POOL_2D => {
                let src = tensors[op.inputs[0] as usize]
                    .clone()
                    .ok_or("pool: entrada no disponible")?;
                let od = &graph.operands[op.outputs[0] as usize];
                let (oh, ow, ch) = (od.dims[1] as usize, od.dims[2] as usize, od.dims[3] as usize);
                let src_h = graph.operands[op.inputs[0] as usize].dims[1] as usize;
                let src_w = graph.operands[op.inputs[0] as usize].dims[2] as usize;
                let pad_l = scalars[op.inputs[1] as usize].unwrap_or(0) as usize;
                let pad_t = scalars[op.inputs[3] as usize].unwrap_or(0) as usize;
                let sw = scalars[op.inputs[5] as usize].unwrap_or(1) as usize;
                let sh = scalars[op.inputs[6] as usize].unwrap_or(1) as usize;
                let fw = scalars[op.inputs[7] as usize].unwrap_or(2) as usize;
                let fh = scalars[op.inputs[8] as usize].unwrap_or(2) as usize;

                let mut out = vec![0i8; oh * ow * ch];
                for y in 0..oh {
                    for x in 0..ow {
                        for c in 0..ch {
                            let mut best = i8::MIN;
                            for dy in 0..fh {
                                for dx in 0..fw {
                                    let iy = (y * sh + dy) as isize - pad_t as isize;
                                    let ix = (x * sw + dx) as isize - pad_l as isize;
                                    if iy < 0 || ix < 0 || iy >= src_h as isize || ix >= src_w as isize
                                    {
                                        continue;
                                    }
                                    // Indexación con offset de canal (NHWC):
                                    // el tensor de entrada del pool tiene `ch` canales.
                                    let v = src[((iy as usize) * src_w + ix as usize) * ch + c];
                                    best = best.max(v);
                                }
                            }
                            out[(y * ow + x) * ch + c] = best.max(0);
                        }
                    }
                }
                tensors[op.outputs[0] as usize] = Some(out);
            }
            other => return Err(format!("operación NNAPI no soportada: {}", other)),
        }
    }

    tensors[graph.output_index as usize]
        .clone()
        .ok_or_else(|| "el grafo no produjo salida".to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Backend real: NNAPI
// ─────────────────────────────────────────────────────────────────────────────

/// Información de ejecución: dice la verdad sobre dónde corrió el trabajo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecInfo {
    /// "nnapi:accelerator" | "nnapi:gpu" | "nnapi:cpu" | "cpu-reference"
    pub backend: String,
    pub device_name: String,
    pub device_type: i32,
    pub device_type_label: String,
    pub nnapi_version: String,
    /// Vacío si no hubo fallback.
    pub fallback_reason: String,
    pub latency_ms: f64,
    /// `true` SÓLO si la ejecución fue realmente sobre hardware NNAPI.
    pub executed_on_hardware: bool,
    pub nnapi_present: bool,
    pub nnapi_device_count: u32,
    pub nnapi_devices: Vec<NnapiDevice>,
}

/// Backend de detección de bordes/estructura de UI.
pub trait EdgeBackend {
    /// Nombre corto del backend.
    fn name(&self) -> &'static str;
    /// Ejecuta el grafo sobre la luma del frame.
    fn run(&self, graph: &GraphDesc, luma: &[u8]) -> Result<Vec<i8>, String>;
}

/// Backend de referencia en CPU (aritmética entera idéntica).
pub struct CpuBackend;

impl EdgeBackend for CpuBackend {
    fn name(&self) -> &'static str {
        "cpu-reference"
    }
    fn run(&self, graph: &GraphDesc, luma: &[u8]) -> Result<Vec<i8>, String> {
        interpret_graph(graph, luma)
    }
}

/// Backend NNAPI real: compila el grafo y lo ejecuta en el dispositivo
/// acelerador (NPU) cuando existe.
pub struct NnapiBackend {
    api: NnapiApi,
    device: NnapiDevice,
    compilation: *mut c_void,
}

impl NnapiBackend {
    /// Intenta crear una compilación en el mejor dispositivo disponible.
    ///
    /// Prioridad: ACCELERATOR (NPU) > GPU > CPU.  Para que el NPU acepte el
    /// trabajo hay que pedir `LOW_POWER` (en Exynos, el NPU sólo se ofrece con
    /// preferencia de bajo consumo; con `SUSTAINED_SPEED` el driver elige la
    /// GPU Mali).
    pub fn open(api: NnapiApi, graph: &GraphDesc) -> Result<(NnapiBackend, ExecInfo), String> {
        let handles = api.device_handles();
        if handles.is_empty() {
            return Err("NNAPI presente pero sin dispositivos".to_string());
        }

        // Prioridad: ACCELERATOR (NPU real) > GPU > CPU.
        let mut chosen: Option<(*mut c_void, NnapiDevice)> = None;
        for want in [DEVICE_ACCELERATOR, DEVICE_GPU, DEVICE_CPU] {
            if let Some(d) = handles.iter().find(|d| d.1.device_type == want) {
                chosen = Some((d.0, d.1.clone()));
                break;
            }
        }
        let (handle, device) = chosen.ok_or("ningún dispositivo NNAPI utilizable")?;

        let mut model: *mut c_void = ptr::null_mut();
        let rc = unsafe { (api.model_create)(&mut model) };
        if rc != 0 || model.is_null() {
            return Err(format!("ANeuralNetworksModel_create rc={}", rc));
        }

        if let Err(e) = unsafe { build_model(&api, model, graph) } {
            unsafe { (api.model_free)(model) };
            return Err(e);
        }
        let rc = unsafe { (api.model_finish)(model) };
        if rc != 0 {
            unsafe { (api.model_free)(model) };
            return Err(format!("ANeuralNetworksModel_finish rc={}", rc));
        }

        // VINCULACIÓN EXPLÍCITA al dispositivo elegido: con
        // `createForDevices` el trabajo va al NPU, no al "mejor" que elija el
        // driver.  La preferencia LOW_POWER reinforce que sea el acelerador.
        let dev_list = [handle];
        let mut compilation: *mut c_void = ptr::null_mut();
        let rc = unsafe {
            (api.compilation_create_for_devices)(
                model,
                dev_list.as_ptr(),
                dev_list.len(),
                &mut compilation,
            )
        };
        if rc != 0 || compilation.is_null() {
            // Fallback: compilación genérica con preferencia LOW_POWER.
            let rc2 = unsafe { (api.compilation_create)(model, &mut compilation) };
            if rc2 != 0 || compilation.is_null() {
                unsafe { (api.model_free)(model) };
                return Err(format!(
                    "ANeuralNetworksCompilation_createForDevices rc={} / create rc={}",
                    rc, rc2
                ));
            }
        }
        unsafe { (api.compilation_set_preference)(compilation, PREFER_LOW_POWER) };
        let rc = unsafe { (api.compilation_finish)(compilation) };
        if rc != 0 {
            unsafe {
                (api.compilation_free)(compilation);
                (api.model_free)(model);
            }
            return Err(format!("ANeuralNetworksCompilation_finish rc={}", rc));
        }
        unsafe { (api.model_free)(model) };

        let info = ExecInfo {
            backend: format!("nnapi:{}", device.type_label.to_lowercase()),
            device_name: device.name.clone(),
            device_type: device.device_type,
            device_type_label: device.type_label.clone(),
            nnapi_version: device.version.clone(),
            fallback_reason: String::new(),
            latency_ms: 0.0,
            executed_on_hardware: true,
            nnapi_present: true,
            nnapi_device_count: handles.len() as u32,
            nnapi_devices: handles.iter().map(|h| h.1.clone()).collect(),
        };

        Ok((
            NnapiBackend {
                api,
                device,
                compilation,
            },
            info,
        ))
    }

    /// Dispositivo NNAPI donde se ejecuta este backend.
    pub fn device(&self) -> &NnapiDevice {
        &self.device
    }

    /// Compila el grafo para una resolución dada.
    pub fn compile(api: NnapiApi, device: NnapiDevice, graph: &GraphDesc) -> Result<Self, String> {
        let mut model: *mut c_void = ptr::null_mut();
        let rc = unsafe { (api.model_create)(&mut model) };
        if rc != 0 || model.is_null() {
            return Err(format!("model_create rc={}", rc));
        }
        if let Err(e) = unsafe { build_model(&api, model, graph) } {
            unsafe { (api.model_free)(model) };
            return Err(e);
        }
        let rc = unsafe { (api.model_finish)(model) };
        if rc != 0 {
            unsafe { (api.model_free)(model) };
            return Err(format!("model_finish rc={}", rc));
        }
        // Vinculación explícita al dispositivo (NPU si es acelerador).
        let mut compilation: *mut c_void = ptr::null_mut();
        let handle = api
            .device_handles()
            .into_iter()
            .find(|h| h.1.name == device.name && h.1.device_type == device.device_type)
            .map(|h| h.0);
        let rc = match handle {
            Some(h) => {
                let list = [h];
                unsafe {
                    (api.compilation_create_for_devices)(
                        model,
                        list.as_ptr(),
                        list.len(),
                        &mut compilation,
                    )
                }
            }
            None => unsafe { (api.compilation_create)(model, &mut compilation) },
        };
        unsafe { (api.model_free)(model) };
        if rc != 0 || compilation.is_null() {
            return Err(format!("compilation_create rc={}", rc));
        }
        unsafe { (api.compilation_set_preference)(compilation, PREFER_LOW_POWER) };
        let rc = unsafe { (api.compilation_finish)(compilation) };
        if rc != 0 {
            unsafe { (api.compilation_free)(compilation) };
            return Err(format!("compilation_finish rc={}", rc));
        }
        Ok(NnapiBackend { api, device, compilation })
    }
}

impl Drop for NnapiBackend {
    fn drop(&mut self) {
        if !self.compilation.is_null() {
            unsafe { (self.api.compilation_free)(self.compilation) };
        }
    }
}

impl EdgeBackend for NnapiBackend {
    fn name(&self) -> &'static str {
        "nnapi"
    }

    fn run(&self, graph: &GraphDesc, luma: &[u8]) -> Result<Vec<i8>, String> {
        let in_desc = &graph.operands[graph.input_index as usize];
        let out_desc = &graph.operands[graph.output_index as usize];
        let in_len = luma.len();
        let out_len = graph.out_h as usize * graph.out_w as usize * 2;

        let mut execution: *mut c_void = ptr::null_mut();
        let rc = unsafe { (self.api.execution_create)(self.compilation, &mut execution) };
        if rc != 0 || execution.is_null() {
            return Err(format!("execution_create rc={}", rc));
        }

        let result = (|| -> Result<Vec<i8>, String> {
            let dims: Vec<u32> = in_desc.dims.clone();
            let in_type = OperandType {
                type_: in_desc.type_,
                dimension_count: dims.len() as u32,
                dimensions: dims.as_ptr(),
                scale: in_desc.scale,
                zero_point: in_desc.zero_point,
            };
            let rc = unsafe {
                (self.api.execution_set_input)(
                    execution,
                    0,
                    &in_type,
                    luma.as_ptr() as *const c_void,
                    in_len,
                )
            };
            if rc != 0 {
                return Err(format!("set_input rc={}", rc));
            }

            let mut out_buf = vec![0i8; out_len];
            let odims: Vec<u32> = out_desc.dims.clone();
            let out_type = OperandType {
                type_: out_desc.type_,
                dimension_count: odims.len() as u32,
                dimensions: odims.as_ptr(),
                scale: out_desc.scale,
                zero_point: out_desc.zero_point,
            };
            let rc = unsafe {
                (self.api.execution_set_output)(
                    execution,
                    0,
                    &out_type,
                    out_buf.as_mut_ptr() as *mut c_void,
                    out_len,
                )
            };
            if rc != 0 {
                return Err(format!("set_output rc={}", rc));
            }

            let rc = unsafe { (self.api.execution_compute)(execution) };
            if rc != 0 {
                return Err(format!("compute rc={}", rc));
            }
            Ok(out_buf)
        })();

        unsafe { (self.api.execution_free)(execution) };
        result
    }
}

/// Traduce la descripción del grafo a llamadas NNAPI reales.
unsafe fn build_model(api: &NnapiApi, model: *mut c_void, graph: &GraphDesc) -> Result<(), String> {
    // Los `dimensions` se copian dentro de `add_operand`, pero mantenemos los
    // vectores vivos durante TODAS las llamadas: un puntero colgante aquí
    // produciría formas basura en el driver (fallo silencioso).
    let dims_store: Vec<Vec<u32>> = graph.operands.iter().map(|o| o.dims.clone()).collect();
    for (idx, od) in graph.operands.iter().enumerate() {
        let dims = &dims_store[idx];
        let optype = OperandType {
            type_: od.type_,
            dimension_count: dims.len() as u32,
            dimensions: if dims.is_empty() { ptr::null() } else { dims.as_ptr() },
            scale: od.scale,
            zero_point: od.zero_point,
        };
        let rc = (api.model_add_operand)(model, &optype);
        if rc != 0 {
            return Err(format!("add_operand rc={}", rc));
        }
    }
    // Valores estáticos: se asignan DESPUÉS de crear todos los operandos
    // (el índice es la posición en el grafo).  Los escalares int32 también
    // viajan por `set_operand_value` (4 bytes little-endian): sin ellos el
    // driver no conoce el padding ni el stride.
    for (idx, od) in graph.operands.iter().enumerate() {
        let payload: Option<Vec<u8>> = match (&od.const_bytes, &od.const_i32) {
            (Some(b), _) => Some(b.clone()),
            (None, Some(v)) => Some(v.iter().flat_map(|x| x.to_le_bytes()).collect()),
            (None, None) => None,
        };
        if let Some(bytes) = payload {
            let rc = (api.model_set_operand_value)(
                model,
                idx as u32,
                bytes.as_ptr() as *const c_void,
                bytes.len(),
            );
            if rc != 0 {
                return Err(format!("set_operand_value({}) rc={}", idx, rc));
            }
        }
    }
    for op in &graph.ops {
        let rc = (api.model_add_operation)(
            model,
            op.op,
            op.inputs.len() as u32,
            op.inputs.as_ptr(),
            op.outputs.len() as u32,
            op.outputs.as_ptr(),
        );
        if rc != 0 {
            return Err(format!("add_operation({}) rc={}", op.op, rc));
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Fachada de alto nivel
// ─────────────────────────────────────────────────────────────────────────────

/// Resultado del análisis de estructura de UI (bordes por banda).
#[derive(Debug, Clone)]
pub struct EdgeResult {
    /// Mapa pooled (i8) con 2 canales: [gx, gy] por celda.
    pub pooled: Vec<i8>,
    pub pooled_w: u32,
    pub pooled_h: u32,
    /// Energía total (suma de valores relu sobre las 2 bandas).
    pub total_energy: i64,
    /// Energía por banda de altura: [arriba, medio, abajo].
    pub band_energy: [i64; 3],
    /// Energía media por celda.
    pub mean_energy: f64,
}

impl EdgeResult {
    /// Densidad de bordes normalizada de una banda (0..1 con 2 canales).
    pub fn band_density(&self, band: usize) -> f64 {
        let (y0, y1) = self.band_rows(band);
        let cells = ((y1 - y0) * self.pooled_w as usize * 2).max(1);
        (self.band_energy[band] as f64 / (cells as f64 * 127.0)).clamp(0.0, 1.0)
    }

    /// Filas del mapa pooled que caen dentro de la banda.
    pub fn band_rows(&self, band: usize) -> (usize, usize) {
        let h = self.pooled_h as usize;
        let (y0, y1) = match band {
            0 => (0, h / 4),
            1 => (h / 4, (h * 2) / 3),
            _ => ((h * 2) / 3, h),
        };
        (y0.min(h), y1.min(h))
    }
}

/// Analiza el frame con el mejor backend disponible.
///
/// Orden de preferencia:
/// 1. NNAPI sobre el dispositivo acelerador (NPU real).
/// 2. Referencia en CPU con aritmética idéntica (si NNAPI no está o falla).
pub fn analyze_edges(luma: &[u8], w: u32, h: u32) -> (EdgeResult, ExecInfo) {
    let graph = build_edge_graph(w, h);
    let t0 = std::time::Instant::now();

    // Intento NNAPI real.
    if let Some(api) = NnapiApi::open() {
        let devices = api.devices();
        let nnapi_present = true;
        let count = devices.len() as u32;
        match NnapiBackend::open(api, &graph) {
            Ok((backend, mut info)) => match backend.run(&graph, luma) {
                Ok(pooled) => {
                    info.latency_ms = round2(t0.elapsed().as_secs_f64() * 1000.0);
                    return (build_result(&graph, pooled), info);
                }
                Err(e) => {
                    let mut info = ExecInfo {
                        backend: "cpu-reference".into(),
                        device_name: String::new(),
                        device_type: 0,
                        device_type_label: "CPU".into(),
                        nnapi_version: String::new(),
                        fallback_reason: format!("NNAPI execution falló: {}", e),
                        latency_ms: 0.0,
                        executed_on_hardware: false,
                        nnapi_present,
                        nnapi_device_count: count,
                        nnapi_devices: devices,
                    };
                    info.latency_ms = round2(t0.elapsed().as_secs_f64() * 1000.0);
                    let pooled = interpret_graph(&graph, luma).unwrap_or_default();
                    return (build_result(&graph, pooled), info);
                }
            },
            Err(e) => {
                let info = ExecInfo {
                    backend: "cpu-reference".into(),
                    device_name: String::new(),
                    device_type: 0,
                    device_type_label: "CPU".into(),
                    nnapi_version: String::new(),
                    fallback_reason: format!("NNAPI no utilizable: {}", e),
                    latency_ms: round2(t0.elapsed().as_secs_f64() * 1000.0),
                    executed_on_hardware: false,
                    nnapi_present,
                    nnapi_device_count: count,
                    nnapi_devices: devices,
                };
                let pooled = interpret_graph(&graph, luma).unwrap_or_default();
                return (build_result(&graph, pooled), info);
            }
        }
    }

    // Sin NNAPI: referencia CPU.
    let info = ExecInfo {
        backend: "cpu-reference".into(),
        device_name: String::new(),
        device_type: 0,
        device_type_label: "CPU".into(),
        nnapi_version: String::new(),
        fallback_reason: format!("libneuralnetworks.so no disponible en {}", NNAPI_LIB_PATH),
        latency_ms: 0.0,
        executed_on_hardware: false,
        nnapi_present: false,
        nnapi_device_count: 0,
        nnapi_devices: Vec::new(),
    };
    let pooled = interpret_graph(&graph, luma).unwrap_or_default();
    let mut info = info;
    info.latency_ms = round2(t0.elapsed().as_secs_f64() * 1000.0);
    (build_result(&graph, pooled), info)
}

/// Agrega el mapa pooled en bandas y métricas.
pub fn build_result(graph: &GraphDesc, pooled: Vec<i8>) -> EdgeResult {
    let pw = graph.out_w as usize;
    let ph = graph.out_h as usize;
    let mut band_energy = [0i64; 3];
    let mut total = 0i64;
    for y in 0..ph {
        let band = if y < ph / 4 {
            0
        } else if y < (ph * 2) / 3 {
            1
        } else {
            2
        };
        for x in 0..pw {
            let base = (y * pw + x) * 2;
            let v = (pooled.get(base).copied().unwrap_or(0)) as i64
                + (pooled.get(base + 1).copied().unwrap_or(0)) as i64;
            band_energy[band] += v;
            total += v;
        }
    }
    let cells = (ph * pw * 2).max(1);
    EdgeResult {
        pooled,
        pooled_w: graph.out_w,
        pooled_h: graph.out_h,
        total_energy: total,
        band_energy,
        mean_energy: (total as f64 / cells as f64 * 100.0).round() / 100.0,
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Sonda de disponibilidad de NNAPI (sin ejecutar nada).
pub fn probe() -> ExecInfo {
    match NnapiApi::open() {
        Some(api) => {
            let devices = api.devices();
            ExecInfo {
                backend: if devices.iter().any(|d| d.device_type == DEVICE_ACCELERATOR) {
                    "nnapi:accelerator".into()
                } else if devices.iter().any(|d| d.device_type == DEVICE_GPU) {
                    "nnapi:gpu".into()
                } else {
                    "nnapi:cpu".into()
                },
                device_name: devices.first().map(|d| d.name.clone()).unwrap_or_default(),
                device_type: devices.first().map(|d| d.device_type).unwrap_or(0),
                device_type_label: devices
                    .first()
                    .map(|d| d.type_label.clone())
                    .unwrap_or_else(|| "UNKNOWN".into()),
                nnapi_version: devices.first().map(|d| d.version.clone()).unwrap_or_default(),
                fallback_reason: String::new(),
                latency_ms: 0.0,
                executed_on_hardware: false,
                nnapi_present: true,
                nnapi_device_count: devices.len() as u32,
                nnapi_devices: devices,
            }
        }
        None => ExecInfo {
            backend: "cpu-reference".into(),
            device_name: String::new(),
            device_type: 0,
            device_type_label: "CPU".into(),
            nnapi_version: String::new(),
            fallback_reason: format!("libneuralnetworks.so no disponible en {}", NNAPI_LIB_PATH),
            latency_ms: 0.0,
            executed_on_hardware: false,
            nnapi_present: false,
            nnapi_device_count: 0,
            nnapi_devices: Vec::new(),
        },
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn pesos_sobel_caben_en_int8() {
        let f = sobel_filters_i8();
        assert_eq!(f.len(), 18);
        assert!(f.iter().all(|v| *v >= -128 && *v <= 127));
        // Los pesos deben ser múltiplos exactos de 32 (representables en 1/32).
        assert!(f.iter().all(|v| (*v as i32) % 32 == 0));
    }

    #[test]
    fn grafo_tiene_operaciones_validas() {
        let g = build_edge_graph(64, 32);
        assert_eq!(g.operands.len(), 22);
        assert_eq!(g.ops.len(), 3);
        assert_eq!(g.ops[0].op, OP_CONV_2D);
        assert_eq!(g.ops[1].op, OP_RELU);
        assert_eq!(g.ops[2].op, OP_MAX_POOL_2D);
        // Cada entrada de op debe existir y tener valor.
        for op in &g.ops {
            for i in &op.inputs {
                assert!((*i as usize) < g.operands.len());
            }
        }
        // Conv VALID (h-2 x w-2) y pool 2x2/stride2 -> (h-4)/2+1
        assert_eq!(g.out_h, 15);
        assert_eq!(g.out_w, 31);
    }

    #[test]
    fn frame_plano_da_energia_cero() {
        let g = build_edge_graph(16, 8);
        let luma = vec![100u8; 16 * 8];
        let out = interpret_graph(&g, &luma).unwrap();
        assert!(out.iter().all(|v| *v == 0), "un frame plano no tiene bordes");
    }
}
