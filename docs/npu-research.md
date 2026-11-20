# Investigación NPU — ¿se puede usar de verdad la NPU de este dispositivo?

> Medición y fuentes. **Nada de lo que sigue es suposición**: cada cifra sale
> de una ejecución real o de una fuente citada. Lo que no se ha podido
> ejecutar en el dispositivo está marcado explícitamente como
> *"verificado por contrato de ABI, no ejecutado"*.

Dispositivo analizado: **Samsung Galaxy S21 Ultra (SM-G998B, `p3s`)**,
**Samsung Exynos 2100** (`ro.soc.model=Exynos 2100`, `ro.board.platform=universal2100_r`),
Android 14 (`UP1A.231005.007`), kernel `5.4.242-28575149-abG998BXXSBGXED`.

---

## 1. Veredicto corto

| Pregunta | Respuesta |
|---|---|
| ¿Hay NPU? | Sí. Exynos 2100: **NPU de triple núcleo, 26 TOPS** (cifra de marketing del SoC). |
| ¿Puede ejecutar el LLM el NPU? | **No.** NNAPI no tiene operaciones de atención ni formas dinámicas; llama.cpp no tiene backend Exynos. |
| ¿Puede ejecutar visión el NPU? | **Sí**, y es lo que hace: INT8 fijo vía NNAPI (`CONV_2D 3×3 → RELU → MAX_POOL_2D 2×2`). |
| ¿Vale la pena usar el NPU para el LLM? | **No**: medido en este mismo S21, la GPU Vulkan hace el decode **2× más lento** que la CPU (22,8–23,1 tok/s vs 46–47 tok/s). |
| ¿Camino correcto del LLM? | **CPU** con `dotprod+i8mm` (+ KleidiAI si la lib está presente), afinidad a cores 4–7. |

---

## 2. Lo que sí funciona: backend NNAPI real

Implementado en `rust-core/src/npu_nnapi.rs` (v13.2):

* **FFI propia**, sin SDK: `dlopen("/system/lib64/libneuralnetworks.so")` en
  runtime, `ANeuralNetworksModel_create`, `ANeuralNetworksCompilation_create`,
  `ANeuralNetworksExecution_create`, `ANeuralNetworksExecution_compute`.
* **Vinculación explícita al dispositivo acelerador**: se enumeran las
  dispositivos NNAPI y se elige el de tipo `DEVICE_ACCELERATOR` (4) con
  `PREFER_LOW_POWER` (0). Si sólo hay CPU/GPU, **no se finge**: se cae a la
  referencia CPU y se informa del motivo.
* **Cuantización INT8 exacta**: `s_in=1.0`, `s_f=1/32`, `s_out=8.0` ⇒
  `|q| ≤ 127` sin saturación en el rango de entrada real.
* **Telemetría honesta**: `executed_on_hardware`, `fallback_reason`,
  `vision_backend`. La UI **nunca** dice "NPU LIVE" si no se ejecutó.

### 2.1 Cómo se ha probado sin el Exynos

En el entorno de desarrollo **no hay** `libneuralnetworks.so`, así que el
camino NNAPI no puede ejecutarse aquí. Para no quedarse en "fantasmadas" se
construyó:

1. **Una implementación independiente de la ABI NNAPI en C**
   (`rust-core/tests/fake_nnapi.c`, ~450 líneas), compilada en runtime con `cc`
   y cargada **con el mismo `dlopen`** que usa el backend real.
2. Una suite de integración (`rust-core/tests/npu_nnapi.rs`, **9 tests**) que
   ejecuta el grafo completo y compara **bit a bit** la salida NNAPI con la
   referencia CPU: **9/9 PASS**, salida idéntica.

Esa prueba encontró **4 bugs de ejecución reales** que una revisión de código
no habría visto:

| Bug | Síntoma | Corrección |
|---|---|---|
| F-32 | La entrada u8 se guardaba como i8 (`230` → `-26`) | Almacenamiento con el tipo del tensor |
| F-33 | El `MAX_POOL_2D` no recibía offset de canal | Offset por canal explícito |
| F-34 | Los escalares int32 (padding/stride) nunca se enviaban a NNAPI | `ANeuralNetworksOperand` escalares |
| F-35 | Puntero colgante a `dimensions` (use-after-free) | Copia propia del buffer |

Además, el padding de la convolución pasó de `SAME` a `VALID`: el
zero-padding inventaba bordes fantasma que la CPU de referencia no tenía.

> **Estado**: el camino NNAPI está verificado **por contrato de ABI** y por
> ejecución real contra una implementación ABI independiente. La ejecución
> sobre el silicio Exynos requiere el dispositivo; el protocolo está en
> `docs/NPU_REAL.md` §3.

### 2.2 Visión continua (para que el NPU no esté ocioso)

El hallazgo que explicaba por qué el NPU estaba infrautilizado: `power_policy.rs`
leía el governor de `policy0` (cluster **pequeño**, que en Android está siempre
en `powersave`) ⇒ `system_power_save=true` casi siempre ⇒ política `saver` ⇒
`allow_heavy=false` ⇒ **la visión nunca arrancaba**.

Correcciones:
* sólo cuentan `policy4/6/7` (cluster grande) o la señal explícita `cpufreq/power_save`;
* `thermal` se evalúa **antes** que `saver`;
* `allow_heavy` dejó de ser decorativo (`/api/run_vla` → `VLA_CV_THROTTLED`).

Resultado: visión continua a **2 Hz** en `performance`, **0,2 Hz** en `saver`,
sólo contador en `critical`. Benchmark A/B en el propio dispositivo:
`GET /api/npu_bench` (misma frame, NNAPI vs CPU, con verificación de
equivalencia y veredicto).

---

## 3. Lo que NO se puede: el LLM en el NPU

### 3.1 Por qué (motivos técnicos, no opinión)

* **NNAPI no tiene operaciones de atención** ni soporte de formas dinámicas:
  un transformer necesita KV-cache dinámico y `Softmax` sobre tensores que
  cambian de forma en cada token.
* **llama.cpp no tiene backend Exynos**: los backends de aceleración de
  llama.cpp cubren CUDA, Vulkan, Metal, SYCL, HIP, CPU (dotprod/i8mm/KleidiAI).
  Exynos se usa **vía NNAPI desde Android**, no desde llama.cpp.
* **Vulkan/Mali como acelerador del LLM: descartado por medición** (abajo).

### 3.2 Medición en este mismo Galaxy S21

Qwen2.5-0.5B-Instruct Q4_K_M, 4 hilos (fuente: measurements del propio
dispositivo, mismo banco que `docs/LLM_SWITCH_PLAN.md`):

| Ruta | Prefill | Decode |
|---|---|---|
| **CPU** (dotprod + i8mm, cores 4–7) | 115–119 tok/s | **46–47 tok/s** |
| GPU Vulkan (Mali) | 227 tok/s | **22,8–23,1 tok/s** |

Conclusión: el prefill mejora en GPU, pero el **decode es 2× más lento**, y en
un agente el decode domina (es un bucle de turnos). Umbral analítico: la GPU
sólo compensa si `prompt > 5,26× salida`. En un episodio SEERE típico
(207 tokens de prompt / 40 de salida) es **empate técnico**: CPU 2629 ms vs
GPU 2644 ms. → **CPU por defecto**, con la decisión calculada en
`GET /api/llm_route` y explicada en la UI.

### 3.3 Precedente que confirma el patrón

En RK3588 (NPU+NPU para LLM, referencia del ecosistema): decode del NPU
**5,3 tok/s** vs CPU **7,4 tok/s**, y sólo con `MUL_MAT`/`MUL_MAT_ID`
soportados; Mali-G610 22× más lento. El patrón "NPU para LLM" no compensa
salvo con kernels específicos y operadores completos.

---

## 4. Camino Snapdragon (para otros dispositivos, NO para este)

El prompt maestro menciona la receta de Qualcomm. **Aquí no aplica**: este
móvil es Exynos. Se documenta igual para que la receta no se pierda:

| Vía | Qué es | Estado para Exynos 2100 |
|---|---|---|
| **SNPE** (Qualcomm Neural Processing SDK) | Runtime de la NPU Hexagon de Qualcomm | **No aplicable** (no es Hexagon) |
| **ExecuTorch + backend QNN** | Orquestador de inferencia con delegate de Qualcomm | **No aplicable** |
| **`avisre/snapdragon-npu-llm`** | Receta para ejecutar LLM en Hexagon incluso en SoC "no soportados" (8 Gen 1 / SM8450); Qwen3-0.6B en `.pte` "SM8450-Hybrid", ~31,3 tok/s y ~107 ms TTFT reportados | **No aplicable** (requiere Hexagon v69+) |
| **NNAPI** | Interfaz estándar de Android | **✅ el camino real en Exynos** |

Regla práctica: **en Android, la NPU se usa a través de NNAPI**; el
vendor-specific SDK sólo aporta cuando el SoC es Qualcomm y hay un
`.pte` compilado para tu Hexagon exacto. En un Exynos, el NPU sólo sirve
para cargas de forma fija (visión, siempre-a-on).

---

## 5. Receita reproducible en el dispositivo

```bash
# 1) ¿Qué dispositivos y NNAPI hay?
adb shell getprop ro.soc.model
adb shell "ls /vendor/lib64/ | grep -i neural"
adb shell "cat /sys/class/nnapi/nnapi*/devices 2>/dev/null"

# 2) Telemetría real de MegaDaemon (sin inventar nada)
adb shell megadaemon-core npu-probe     # dispositivos NNAPI, dotprod, KleidiAI
adb shell megadaemon-core npu-cv        # backend de visión + mapa de calor

# 3) Benchmark A/B en el propio chip
curl -H "X-API-Key: $KEY" http://127.0.0.1:9090/api/npu_bench?iters=20
# -> compara latencia NNAPI vs CPU y verifica que las SALIDAS COINCIDEN

# 4) Si sale "fallback: libneuralnetworks.so no disponible",
#    el dispositivo no expone NNAPI: no se puede usar el NPU (y la UI lo dice).
```

### 5.1 Flags de compilación del LLM (para Exynos)

```
GGML_CPU_ARM_ARCH=armv8.2-a+dotprod+i8mm
GGML_CPU_KLEIDIAI=ON        # off por defecto; requiere AArch64 y KleidiAI >= 1.32
-t 4                        # afinidad a los cores 4-7 (1x X1 + 3x A78)
```

---

## 6. Resumen ejecutivo para el informe

1. La NPU **sí se usa** en este dispositivo, pero **para visión INT8 de forma
   fija**, vía NNAPI, con binding explícito al dispositivo acelerador y
   telemetría honesta (`executed_on_hardware`).
2. El **LLM no puede ir al NPU** (sin ops de atención, sin formas dinámicas,
   sin backend Exynos en llama.cpp).
3. La **GPU no sirve para el LLM**: medido 2× más lento en decode.
4. La ruta óptima es **CPU con dotprod+i8mm**, y la decisión se calcula en
   runtime (`/api/llm_route`) en lugar de estar codificada a mano.
5. El NPU estaba infrautilizado por un bug de la **política de energía**
   (leía el cluster equivocado) — corregido, con visión continua y benchmark
   A/B para demostrarlo en el dispositivo.

**Fuentes**: ETH Zürich AIM'21 (NPU Exynos 2100 = INT8 vía NNAPI; GPU =
INT8/FP16 vía Eden+NNAPI) · arXiv 2410.03613 · `llama.cpp#22682` ·
documentación de llama.cpp (`GGML_CPU_KLEIDIAI`, `GGML_CPU_ARM_ARCH`) ·
mediciones propias en Galaxy S21 (Exynos 2100) documentadas en
`docs/LLM_SWITCH_PLAN.md`.
