# NPU real en MegaDaemon v13.2 — investigación, diseño, implementación y límites

> Documento de referencia del camino de offload a acelerador.
> Todo lo que aquí se afirma está respaldado por (a) una prueba ejecutable del
> repo, (b) una medición publicada con su fuente, o (c) una verificación por
> contrato de ABI con el header oficial de AOSP. Cuando algo **no** se ha
> podido ejecutar, se dice explícitamente.

---

## 0. Resumen ejecutivo (el veredicto, sin adornos)

**Pregunta:** ¿se puede usar de verdad el NPU del Exynos 2100 en MegaDaemon?

**Respuesta:** **Sí, para la visión. No, para el LLM. Y la GPU no sirve para el LLM en este chip.**

| Carga | ¿Se acelera? | Con qué | Evidencia |
|---|---|---|---|
| Detección de estructura de UI (bordes) | **Sí, implementado en v13.2** | NPU vía **NNAPI INT8** (`rust-core/src/npu_nnapi.rs`) | 8 pruebas, incluida una que ejercita el camino FFI completo contra una implementación independiente de la ABI |
| LLM decode | **No en NPU** | **CPU** (Cortex-X1 + 3×A78, dotprod+i8mm+KleidiAI) | El NPU no puede: NNAPI no tiene ops de atención ni formas dinámicas; llama.cpp no tiene backend Exynos |
| LLM prefill | **No en NPU** | CPU (mismos 4 núcleos) | Ídem |
| LLM en GPU Vulkan (Mali-G78) | **No, es contraproducente** | CPU | Medido en el **propio Galaxy S21**: GPU 22,8–23,1 tok/s vs CPU 46–47 tok/s de decode |
| YOLO / OCR / CLIP (visión pesada) | **Todavía no** | — | Falta un modelo INT8 cuantizado + NMS; no se promete |
| Nodo `/dev/vertex10` presente | **No prueba nada** | — | La v13.0 lo.fetchaba como si probara el offload; ahora sólo es un dato |

La frase que resume el cambio de fondo:

> En v13.0 el "NPU" era **telemetría decorativa**: un string que prometía
> "Prefill Batch → Exynos NPU (EDEN/NNAPI)" mientras **ningún módulo** invocaba
> EDEN, VNNI ni NNAPI. En v13.2 hay un **grafo NNAPI real** que se compila y
> ejecuta, con fallback verificable y telemetría que dice la verdad.

---

## 1. Investigación: qué es realmente el NPU del Exynos 2100

### 1.1 El NPU es INT8-only

Fuente primaria: **ETH Zürich, "Deep Learning on Mobile Devices: An In-Depth
Overview"** (Ignatov et al., AIM'21,
<https://data.vision.ee.ethz.ch/cvl/aim21/slides/Andrey-Ignatov-AIMTalk-10-16-2021.pdf>):

> Samsung Chipsets: NN Inference Acceleration
> **NPU**: INT8 NN inference support — Accessible through **NNAPI** — Exynos 2100, 990, 980…
> **GPU**: INT8 & FP16 NN inference support — Accessible through the **Eden** TFLite delegate and NNAPI — Exynos 2100, 990, 9825…

**Consecuencia de diseño (la más importante de todo el documento):** un grafo
NNAPI `float32` **nunca** tocaría el NPU del Exynos 2100; el driver lo
ejecutaría en la GPU Mali o en la CPU. Por eso **todo** el módulo
`npu_nnapi.rs` es `TENSOR_QUANT8_ASYMM` de principio a fin. Un backend NNAPI
"bonito" en float32 habría sido exactamente la misma fantasmada que la v13.0,
sólo que con más código.

### 1.2 Qué es NNAPI y por qué es la única vía

NNAPI (Android Neural Networks API) es la API **pública de Android** (NDK, API
27+) que multiplexa los dispositivos de aceleración del sistema: CPU, GPU y
"ACCELERATOR" (el NPU del fabricante).

* Documentación oficial: <https://source.android.com/docs/core/interaction/neural-networks>
* La API NDK es *deprecated* desde Android 15, pero **el HAL sigue soportado**
  → no es una vía que vaya a desaparecer.
* La ruta de código es `/system/lib64/libneuralnetworks.so` (stub NDK) →
  binder → HAL del fabricante (`android.hardware.neuralnetworks@1.x-service`).
* Samsung expone su NPU a través de este HAL (y también con el *delegate*
  "Eden" de TFLite, que es la vía TFLite equivalente a la misma pila).

No hay "SDK del NPU de Samsung" público: la vía soportada es NNAPI.

### 1.3 Por qué llama.cpp no puede usar el NPU

* Los backends de ggml/llama.cpp son: **CANN, CUDA, HIP, Hexagon, MUSA, Metal,
  OpenCL, OpenVINO, RPC, SYCL, Vulkan, WebGPU, ZenDNN, CPU (KleidiAI/NVNPT)**.
  **No hay backend Exynos/NPU.** (README de llama.cpp, sección *Supported
  backends*.)
* **Hexagon** (Qualcomm) es el único NPU con backend, y el Exynos no lo es.
* NNAPI **no define operaciones de atención** ni admite **formas dinámicas**
  (el KV-cache de un LLM cambia de longitud en cada token) → aunque existiera
  un backend, el modelo no se podría expressesar en NNAPI.

### 1.4 El precedente más cercano: RK3588

El único caso real de "NPU ejecutando un LLM" documentado es el RK3588
(Rockchip) con un fork de llama.cpp (`rk-llama.cpp`). Sus propias mediciones:

| Configuración | Prefill | Decode |
|---|---|---|
| CPU (W8A8 + FA) | 12,7 t/s | **7,4 t/s** |
| 10 capas en NPU (int8) | 18,2 t/s | **5,3 t/s** |
| 48 capas en NPU (int8) | 26,6 t/s | **2,5 t/s** |

> "The NPU implements **only** `MUL_MAT` and `MUL_MAT_ID`… Everything else runs
> on the CPU, so a token crosses between the two hundreds of times."

Es decir: **el NPU acelera el prefill pero empeora el decode**, porque cada
token genera ~1152 transferencias CPU↔NPU. La lección se transfiere directamente
al Exynos: aunque existiera backend, el LLM **no** debe ir al NPU.

Fuente: <https://github.com/Mojo24x7/rk-llama.cpp/blob/rknpu2-current/BENCHMARKS.md>

### 1.5 La GPU Mali: mide, no opines

En el **Galaxy S21 (Exynos 2100, Mali-G78 MP14)** con Qwen2.5-0.5B-Instruct
Q4_K_M, 4 hilos, medición física (ameva-runtime v2.7.9, 2026-10-04):

| Route | Prompt processing | Token generation |
|---|---|---|
| **CPU** | 115–119 t/s | **46–47 t/s** |
| **GPU (Vulkan/Mali)** | 227 t/s | **22,8–23,1 t/s** |

* El **prefill** mejora 2× en GPU, pero el **decode se年半** (46 → 23 t/s).
  Como el LLM es *decode-bound* (el prompt del SEERE son 187–207 tokens y la
  respuesta 40–64), **la GPU es una pérdida neta**.
* En la misma familia: **Galaxy A53 (Exynos 1280, Mali-G68)**: CPU 22–24 t/s
  vs GPU 14,5–15 t/s de decode.
* El propio estudio de la AOSP/ETH razona la causa: *"On the Mali devices token
  generation is faster on the CPU, and Galaxy S21 and A53, whose drivers lack
  integer dot products, take the CPU route by default."* → **el driver Vulkan
  de Mali no expone las instrucciones enteras dotprod**, justo las que usa el
  kernel `Q4_0_8x8` de llama.cpp.

Otros Mali (mismo patrón): rk3588 (Mali-G610) → **22× más lento** que la CPU
(OpenCL) y Vulkan con segfaults; llama.cpp#22682 (ARM Mali) → CPU
35,9/25,7 t/s vs Vulkan **3,3/5,7 t/s**.

Fuentes:
* <https://pypi.org/project/ameva-runtime/> (tabla de mediciones por dispositivo)
* <https://arxiv.org/html/2410.03613> ("Understanding Large Language Models in
  Your Pockets: Performance Study on COTS Mobile Devices") — *"Among the tested
  GPUs, Mali GPUs, particularly the Mali-G78, exhibit significantly slow
  prefill speeds"* y *"GPUs do not exhibit a clear advantage in practice"*.
* <https://github.com/ggml-org/llama.cpp/issues/22682>

### 1.6 Qué sí acelera de verdad el LLM en este SoC

1. **dotprod + i8mm**: el Cortex-X1 y los A78 son Armv8.2 → `asimddp` ✓.
   llama.cpp detecta el feature y compila `armv8.2-a+dotprod` automáticamente,
   pero se puede fijar: `-DGGML_CPU_ARM_ARCH=armv8.2-a+dotprod+i8mm`.
2. **KleidiAI**: existe en llama.cpp (`-DGGML_CPU_KLEIDIAI=ON`, kernels ARM
   para SDOT/INT8, requiere AArch64 y CMake ≥ 3,16; descarga KleidiAI v1.32.0).
3. **Afinidad a los núcleos grandes** (cores 4–7 = 1×X1 + 3×A78) y `-t 4`
   (más hilos que núcleos grandes *empeoran*: en el RK3588, `-t 8` costaba un
   56 % por la barrera big.LITTLE).
4. **Coste real = ancho de banda de memoria**: el decode de un 0,5B Q4_K_M
   (398 MB) está limitado por la DRAM, no por la ALU. Ése es el motivo por el
   que el NPU (que tiene su propia SRAM) no ayuda al decode.

---

## 2. Lo implementado en v13.2

### 2.1 Ficheros

| Fichero | Qué hace |
|---|---|
| `rust-core/src/npu_nnapi.rs` | Backend NNAPI real: FFI, `dlopen`, grafo INT8, ejecución, fallback y telemetría |
| `rust-core/src/accelerator_policy.rs` | Política REAL de aceleradores (JSON tipado) con el motivo de cada veredicto |
| `rust-core/src/npu_cv.rs` | Integra el backend en la tubería de visión (una sola algoritmo, dos ejecutores) |
| `rust-core/src/npu_bridge.rs` | Telemetría: `npu_offload_implemented: true` con el alcance exacto + política |
| `rust-core/tests/fake_nnapi.c` | Implementación independiente de la ABI NNAPI en C (para testear el FFI sin hardware) |
| `rust-core/tests/npu_nnapi.rs` | 8 pruebas del backend |

### 2.2 El grafo

```text
frame SHM (RGB)
   │  luma: (77·R + 150·G + 29·B) >> 8
   ▼
input  TENSOR_QUANT8_ASYMM [1,H,W,1]  scale 1.0   zp 0     (u8, 0..255)
   │
   ├─ CONV_2D 3×3, stride 1, VALID, 2 canales
   │    filtro = [Sobel X | Sobel Y]  TENSOR_QUANT8_ASYMM  scale 1/32  zp 0
   │    bias   = TENSOR_INT32[2]  scale 1/32
   │    → salida TENSOR_QUANT8_ASYMM [1,H-2,W-2,2]  scale 8.0  zp 0
   ├─ RELU   (clamp 0..127)
   ├─ MAX_POOL_2D 2×2, stride 2, VALID
   │    → salida TENSOR_QUANT8_ASYMM [1,·,·,2]  scale 8.0  zp 0
   ▼
agregación por banda en CPU: top / medio / abajo → edge_density por ROI
```

**Cuantización (exacta, sin flotantes en el camino crítico):**

```text
q_x   = luma                         (u8, s_in = 1.0,  zp = 0)
q_w   = w · 32                       (i8, s_f = 1/32, zp = 0)   → Sobel exacto: ±32, ±64
acc   = Σ q_x·q_w + bias             (i32, escala s_in·s_f = 1/32)
real  = acc · (s_in·s_f)             = Σ luma·w        (respuesta Sobel real)
q_out = clamp(round(real / 8.0), -128, 127)
relu  = clamp(q_out, 0, 127)
pool  = max 2×2
```

Elección de escalas: con `s_out = 8.0` y luma en [0,255], la respuesta Sobel
máxima (4·255 = 1020) mapea a **127**, el máximo exacto de int8. Los pesos son
exactos en int8 (multiplos de 32). La escala de salida la declara el *modelo*,
no el driver: es la "requantización" que define la spec de NNAPI para conv
cuantizada.

**Padding VALID (no cero):** con zero-padding el marco del frame produce un
"borde" artificial que contamina la energía de la banda superior. Se detectó
midiendo: el frame plano daba energía ≠ 0. Con VALID, un frame plano da
energía **exactamente 0** (hay una prueba para ello).

### 2.3 La ABI (verificada contra AOSP)

Verificada contra `nnapi-sys` 0.1.1 (bindings generados del header AOSP
`neural_networks.h`), crate: <https://crates.io/crates/nnapi-sys>

```c
typedef struct {
    int32_t  type;            // OperandCode: TENSOR_QUANT8_ASYMM = 5
    uint32_t dimensionCount;  // 0 para escalares
    const uint32_t *dimensions;
    float    scale;           // escala de cuantización
    int32_t  zeroPoint;
} ANeuralNetworksOperandType;
```

Operaciones usadas (valores del header): `CONV_2D = 3`, `MAX_POOL_2D = 17`,
`RELU = 19`; `FUSE_NONE = 0`; `PADDING_SAME = 1`, `PADDING_VALID = 2`;
`PREFER_LOW_POWER = 0`; `DEVICE_CPU = 2`, `DEVICE_GPU = 3`,
`DEVICE_ACCELERATOR = 4`.

Funciones de la NDK empleadas (todas verificadas por firma):

```text
ANeuralNetworks_getDeviceCount(uint32_t*)            ANeuralNetworksModel_create(ANeuralNetworksModel**)
ANeuralNetworks_getDevice(uint32_t, ANeuralNetworksDevice**)  ANeuralNetworksModel_addOperand(model, const OperandType*)
ANeuralNetworksDevice_getName / getVersion / getType          ANeuralNetworksModel_setOperandValue(model, idx, buf, len_bytes)
                                                          ANeuralNetworksModel_addOperation(model, type, nin, in[], nout, out[])
                                                          ANeuralNetworksModel_finish / _free
                                                          ANeuralNetworksCompilation_create(model, comp**)
                                                          ANeuralNetworksCompilation_createForDevices(model, devs[], n, comp**)
                                                          ANeuralNetworksCompilation_setPreference(comp, pref)
                                                          ANeuralNetworksCompilation_finish / _free
                                                          ANeuralNetworksExecution_create(comp, exec**)
                                                          ANeuralNetworksExecution_setInput(exec, idx, type, buf, len_bytes)
                                                          ANeuralNetworksExecution_setOutput(exec, idx, type, buf, len_bytes)
                                                          ANeuralNetworksExecution_compute / _free
```

Decisiones de ingeniería:

* **`dlopen` en runtime**, no dependencia de link-time: el crate debe compilar
  en Linux/CI sin `libneuralnetworks.so`.
* **`createForDevices` con el handle del acelerador**: la vinculación
  explícita garantiza que el trabajo va al NPU, en vez de dejar que el driver
  elija "el mejor" (que en Exynos con `SUSTAINED_SPEED` es la GPU Mali).
  `PREFER_LOW_POWER` se fija además como refuerzo.
* **Longitudes en BYTES** en `set_operand_value` / `set_input` / `set_output`
  (no en elementos): es lo que dice el header, y es un error fácil de cometer.
* **Los escalares int32 también viajan por `set_operand_value`** (4 bytes
  little-endian): sin ellos el driver no conoce el padding ni el stride.
  *(Bug encontrado por la prueba FFI: en la primera versión no se enviaban.)*

### 2.4 La prueba que valida el camino FFI de verdad

No basta con "compila": se construyó **una implementación independiente de la
ABI NNAPI en C** (`rust-core/tests/fake_nnapi.c`, ~330 líneas) que:

* reproduce el layout del `OperandType` y la semántica de cada función
  (incluida la copia de las dimensiones, como hace el NDK),
* evalúa el grafo con la misma aritmética entera,
* **escribe la salida en el buffer del llamante** (in-place, como el NDK).

El test compila esa `.so` en runtime con `cc` y la carga con el **mismo
`dlopen` que usaría el móvil**. Así se verifica, sin hardware:

* el orden de argumentos de cada función,
* el layout del `OperandType` (si los campos estuvieran desplazados, las
  formas serían basura),
* las longitudes en bytes,
* los índices del grafo,
* y que **la ruta NNAPI y la referencia CPU dan el mismo resultado bit a bit**.

### 2.5 Los 4 bugs reales que encontró esa prueba

| # | Bug | Síntoma | Gravedad |
|---|---|---|---|
| 1 | La entrada u8 se guardaba en un buffer i8 | luma 230 → −26; el detector perdía los bordes brillantes | **Alta**: en el móvil la detección de UI habría fallado en silencio |
| 2 | El pool indexaba sin el offset de canal | energía leída de píxeles equivocados | **Alta** |
| 3 | Los escalares (padding/stride) nunca se enviaban | el driver no podía compilar el modelo | **Alta**: el modelo se habría rechazado |
| 4 | `dimensions` era un puntero colgante (Vec temporal) | formas basura en el driver | **Alta**: corrupción silenciosa |

Cada uno de ellos habría sido invisible en una revisión de código normal: sólo
aparecen al ejecutar el camino FFI completo contra una implementación real de
la API. Éste es el argumento de por qué el test existe.

### 2.6 Telemetría honesta

Campos nuevos en `/api/npu_cv` y en `npu-probe`:

```json
{
  "vision_backend": "nnapi:accelerator | nnapi:gpu | nnapi:cpu | cpu-reference",
  "vision_executed_on_hardware": true,      // true SÓLO si se ejecutó en hardware
  "vision_fallback_reason": "",             // motivo si hubo fallback
  "vision_latency_ms": 3.7,
  "edge_energy_model": "int8 conv3x3-sobel -> relu -> maxpool2x2 (NHWC)",
  "nnapi_device_count": 2,
  "nnapi_devices": [ { "name": "...", "device_type": 4, "type_label": "ACCELERATOR (NPU)" } ]
}
```

Y en la telemetría de hardware, la clasificación en tres listas (lo que **se
ejecuta**, lo que **sería posible**, lo que **es imposible**):

* `npu_workloads_implemented`: la tubería INT8 por NNAPI.
* `npu_workloads_possible`: YOLO/SSD/OCR/CLIP INT8 (requiere modelo + NMS).
* `npu_workloads_impossible`: el LLM en el NPU, y Vulkan/Mali para el LLM.

En la UI web, el *pill* de la cabecera ya no dice "NPU LIVE" porque exista
`/dev/vertex10`: dice **NPU ACCEL / GPU NNAPI / CPU FALLBACK** según el
backend que realmente ejecutó la última análise.

---

## 3. Protocolo de verificación en el dispositivo (Galaxy S21 / Exynos 2100)

```bash
# 1) ¿Existe la API NNAPI y qué dispositivos expone?
adb shell ls -l /system/lib64/libneuralnetworks.so
adb shell getprop ro.build.version.sdk          # 29/30/31 → NNAPI 1.2/1.3

# 2) ¿El vendor tiene servicio NNAPI (el NPU de Samsung)?
adb shell ls /vendor/bin/hw/ | grep -i neural
adb shell ps -A | grep -i neuralnetworks

# 3) ¿Qué ejecuta MEGADAEMON? (debe decir nnapi:accelerator)
adb shell su -c '/data/user/0/com.agy/files/usr/bin/megadaemon-core npu-cv'
adb shell su -c '... megadaemon-core npu-probe' | jq .npu_vision_backend

# 4) Telemetría del sistema durante la ejecución
adb logcat -s NNAPI:V ANeuralNetworks:V | tail -50

# 5) Prueba de stressors del driver (opcional, requiere root)
adb shell su -c 'cat /sys/kernel/debug/nnapi/...'
```

**Criterio de aceptación** (el mismo que se aplicó a todo lo demás):

| Métrica | Umbral |
|---|---|
| `vision_backend` | `nnapi:accelerator` |
| `vision_executed_on_hardware` | `true` |
| `vision_fallback_reason` | vacío |
| Latencia de visión | **<** latencia de la referencia CPU (el NPU debe ganar) |
| Energía por banda | equivalente (±1 LSB por el redondeo del driver) |

Si el driver de Samsung rechazase el modelo (p. ej. por una op no soportada),
el fallback es **automático** y la telemetría lo declara: no hay Mentiras.

---

## 4. Limitaciones Honestas (lo que NO se ha hecho)

1. **La ejecución en el NPU real no se ha ejecutado aquí**: este entorno no
   tiene hardware Exynos ni `libneuralnetworks.so`. Lo verificado es (a) el
   contrato de ABI contra el header AOSP, (b) el camino FFI completo contra
   una implementación independiente de la spec. La primera vez que corra en el
   S21, el criterio de aceptación de §3 dirá si el driver de Samsung acepta el
   grafo.
2. **La equivalencia numérica con el driver real es ±1 LSB**: el redondeo de
   la requantización del driver puede ser "nearest, ties to even" en lugar de
   "ties away from zero". La prueba exige **igualdad exacta** entre la ruta
   NNAPI y la referencia CPU *de nuestro propio código*; frente al driver real
   la tolerancia debe ser de 1 LSB (8 unidades de luma).
3. **No hay detector de objetos**: el NPU hace estructura (bordes), no
   detección. YOLO/SSD/MobileNet INT8 exigiría el modelo cuantizado, NMS en
   CPU y un gestor de memoria del NPU; no se ha hecho y no se promete.
4. **El LLM no va al NPU** — y no es una limitación de implementación, es una
   limitación de la API y del silicio (ver §1.3–§1.4).
5. **No se ha medido el NPU**: ninguna cifra de tokens/s o de FPS del NPU
   aparece en este documento, porque no se ha ejecutado en el hardware. Las
   únicas cifras son las publicadas por terceros, con su fuente.

---

## 5. Bibliografía

1. Ignatov, A. et al. — *Deep Learning on Mobile Devices: An In-Depth
   Overview*, ETH Zürich, AIM'21.
   <https://data.vision.ee.ethz.ch/cvl/aim21/slides/Andrey-Ignatov-AIMTalk-10-16-2021.pdf>
   → NPU Exynos 2100 = INT8 vía NNAPI; GPU = INT8/FP16 vía Eden + NNAPI.
2. AOSP — *Neural Networks API drivers*.
   <https://source.android.com/docs/core/interaction/neural-networks>
   → HAL 1.0–1.3, deprecación de la NDK en Android 15, operaciones por versión.
3. `nnapi-sys` 0.1.1 (bindings del header AOSP `neural_networks.h`).
   <https://crates.io/crates/nnapi-sys>
4. ggml-org/llama.cpp — README, sección *Supported backends* (no hay Exynos).
   <https://github.com/ggml-org/llama.cpp>
5. ggml-org/llama.cpp — `ggml/src/ggml-cpu/CMakeLists.txt`
   (`GGML_CPU_KLEIDIAI`, `GGML_CPU_ARM_ARCH`, detección dotprod/i8mm).
6. ameva-runtime v2.7.9 — mediciones físicas por dispositivo (S21 Exynos 2100:
   CPU 46–47 tok/s decode vs GPU 22,8–23,1 tok/s).
   <https://pypi.org/project/ameva-runtime/>
7. *Understanding Large Language Models in Your Pockets: Performance Study on
   COTS Mobile Devices*, arXiv:2410.03613.
   <https://arxiv.org/html/2410.03613>
8. ggml-org/llama.cpp issue #22682 — *vulkan: Poor performance on ARM iGPUs*.
   <https://github.com/ggml-org/llama.cpp/issues/22682>
9. Mojo24x7/rk-llama.cpp — *BENCHMARKS.md* (NPU RK3588: `MUL_MAT`/`MUL_MAT_ID`
   solamente; decode más lento en NPU que en CPU; Mali-G610 22× más lento).
   <https://github.com/Mojo24x7/rk-llama.cpp/blob/rknpu2-current/BENCHMARKS.md>
10. ONNX Runtime — *NNAPI Execution Provider* (semántica de la conv
    cuantizada, flags de dispositivo).
    <https://onnxruntime.ai/docs/execution-providers/NNAPI-ExecutionProvider.html>
11. Arm — *Integrator Guide* del driver NNAPI (referencia de la arquitectura
    HIDL del HAL).
    <https://android.googlesource.com/platform/external/android-nn-driver/>

---

*Documento generado en la auditoría v13.2 de MegaDaemon. Todas las cifras de
rendimiento citadas son de terceros y van con su fuente; ninguna es una
medición propia de este repositorio.*
