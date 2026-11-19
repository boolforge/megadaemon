# MegaDaemon v13.0 → v13.1 — Auditoría adversarial, correcciones y hoja de ruta

**Repo auditado:** `github.com/boolforge/megadaemon` (commit `77f5ae4`, "v13.0-zenith").
**Superficie:** 45 ficheros Python (10 037 líneas) + 31 Rust (11 146) + 2 Java (102) + 1 HTML (1 707).
**Método:** lectura de fuente → auditoría con evidencia ejecutable → corrección → re-medición contra un modelo real.
**Entorno de verificación:** Rust 1.100.0, `cargo build` limpio, `cargo test` con **21/21** pruebas adversarias,
llama-server b12202 + **Qwen2.5-0.5B-Instruct Q4_K_M real** para el E2E del agente local.

> ⚠️ **El token de GitHub que pegaste en el chat** (`ghp_mRZgn…`) queda en el historial de la conversación.
> El repo es **público** y se clonó **sin usar el token**, así que no hizo falta para nada.
> **Revócalo ya** en <https://github.com/settings/tokens> y revoca ese `ghp_`.

---

## 0. Veredicto

**"Apareces como minimax 2.7"** no es un bug de estilo: **es un modelo muerto**.

> MiniMax retiró **M2.5 y M2.7 el 18-nov-2026** — ayer. El daemon tenía `minimax-m2.7` como id por
> defecto en el MCP, en el router Python, en el HTML del cockpit y en el banner del servidor.
> **Todas las peticiones al proveedor por defecto de MegaDaemon estaban 返回ando error.**
> Y `GitHub Models` (el otro proveedor "gratuito" estrella del README) **fue retirado el 30-jul-2026**:
> `gh auth token` sigue funcionando, pero no hay ningún modelo detrás.

Lo segundo: **"lo del LLM local está fatal"** era literal. El agente local en Rust tenía el defecto
que yo mismo había medido en la auditoría de SEERE (el fingerprint del host viajando al prompt del
modelo). Medido contra el 0.5B real, en el mismo servidor y con las mismas 12 preguntas:

| Agente local Rust | enrutado | **corrección semántica** | fundamentado |
|---|---|---|---|
| v13.0 (con fingerprint `<ENV>`) | 6/12 | **3/12** | 6/12 |
| **v13.1 (sin fingerprint)** | **8/12** | **8/12** | **9/12** |

**×2,7 en corrección semántica** sin tocar el modelo, sin tocar el prompt de fondo y sin un solo token
de nube.

---

## 1. Los 20 hallazgos

Todos con evidencia ejecutable (test o medición), ordenados por impacto.

| # | Hallazgo | Evidencia | Estado |
|---|---|---|---|
| **F01** | El modelo por defecto `minimax-m2.7` está **RETIRADO** (18-nov-2026) | Docs MiniMax: retirada el 18-nov-2026 | ✅ catálogo v13.1 |
| **F02** | `GitHub Models` **RETIRADO** (30-jul-2026) pero sigue en README, MCP y router | Docs: servicio retirado | ✅ retirado del catálogo |
| **F03** | El fingerprint del host (`<ENV uid=…>`) viaja al prompt del modelo | E2E real: sens 3/12 → 8/12 al quitarlo | ✅ corregido |
| **F04** | El modelo copia la línea `OPS` como si fuera una orden (`A: $ OPS $ shell \| ? web search`) | 2-7 de 19 respuestas en los 4 modelos | ✅ guardián anti-eco |
| **F05** | El pipeline `cat x \| $ grep y` se parte en 2 acciones y ejecuta basura | 10-16 de 19 respuestas | ✅ normalización |
| **F06** | La respuesta `R: $ free -m` (una orden, no una respuesta) se aceptaba como respuesta | 2 de 4 respuestas finales del 1B | ✅ acción de última hora |
| **F07** | El mensaje de reparación ("Unsupported literals […]") podía **convertirse en la respuesta** | E2E: la respuesta final era literalmente el mensaje de error | ✅ rechazada |
| **F08** | Una cita `[SRC:n]` que no resuelve a ninguna observación pasaba la verificación | `verify_grounding` no miraba las citas | ✅ cita debe resolver |
| **F09** | Sin observaciones, cualquier número de la respuesta se daba por bueno | "Esta máquina tiene 5 núcleos" → `grounded=true` | ✅ sin OBS, número = inventado |
| **F10** | "4" casaba **dentro** de `pid=1414` (matching por subcadena) | Test `d4_matching_por_limite_de_palabra` | ✅ límites de palabra |
| **F11** | "5.10.144 MB" (un tamaño) no se comprobaba: la regla de "versiones" lo filtraba | Test `d5_numero_con_unidad_es_dato` | ✅ números con unidad |
| **F12** | El cliente HTTP del agente tenía host/puerto **hardcodeados** a `127.0.0.1:8080` | Ignoraba `SEERE_LLM` | ✅ configurable |
| **F13** | `Connection: close` + `read_to_end`: si el servidor no cerraba, **cada llamada comía 25 s** | Código + falta de test | ✅ lee Content-Length / chunked |
| **F14** | El status HTTP **nunca se miraba**: un 401/404 se parseaba como respuesta vacía | `parse_model_output` con `""` | ✅ 401/402/404/429 diferenciados |
| **F15** | **Cero reintentos**: un solo RST de TCP mataba el episodio | `call_llama_oai` sin retry | ✅ 3 intentos con backoff |
| **F16** | `id_slot: 0` en **todas** las peticiones: serializa lo concurrente | llama.cpp ya elige el slot con mayor prefijo común | ✅ slot configurable |
| **F17** | La acción truncada se completaba **sin gramática** → la degeneración se ejecutaba | "A: K s 1.5K.1.1.0.0.1.0.2.0.0.0.8.0" (1B) | ✅ truncar = descartar |
| **F18** | El fallback final devolvía una **observación cruda** como si fuera la respuesta | `obs.last()` | ✅ marcado `answer_is_observation` |
| **F19** | `providers.json` corrupto → `save_vault()` **sobrescribía y perdía todas las API keys** | `load_vault` devolvía el default | ✅ copia `.corrupt-<ts>` |
| **F20** | `gh auth token` se ejecutaba en **cada** petición (fork+exec por request) | `resolve_gh_token()` sin caché | ✅ caché 300 s |

Hallazgos menores incluidos en el arreglo:

| # | Hallazgo | Estado |
|---|---|---|
| **F21** | La API key se escribía a **disco** en `/data/user/0/…/tmp/md_curl_*.cfg` (ruta fija); si el write fallaba, la petición salía **sin cabecera de auth** y el error decía "transport error" en vez de "401" | ✅ TLS nativo (ureq+rustls): la key no toca el disco |
| **F22** | `curl` como dependencia: **no existe en el toybox de AOSP** (el agente no podía hablar con la nube sin Termux) | ✅ cliente HTTPS nativo |
| **F23** | Los opcodes RE (D/F/X) inflaban el prompt del 0.5B: **227 → 207 tokens**, y la corrección semántica bajaba **8/12 → 6/12** | ✅ ocultos por defecto (`re_opcodes`) |
| **F24** | El catálogo por defecto tenía **2 entradas** (local + pollinations) aunque el README-annunciaba 9 proveedores | ✅ catálogo de 18 |
| **F25** | `gh_cli`/`GitHub Models` era el "free" estrella del README: **servicio extinguido** | ✅ sustituido por Groq/OpenRouter/Google |
| **F26** | El HTML tenía `"minimax-m2.7"` hardcodeado como default del selector y en la cabecera | ✅ `local-seere` + aviso de retirados |
| **F27** | `read_cpu_and_thermal` empujaba las frecuencias **sólo de los cores leídos** → la UI atribuía la frecuencia de un core a otro si el core0 estaba offline | ✅ vector indexado por core |
| **F28** | El thermal tomaba el **max de las 8 primeras zonas sin mirar su tipo**: el sensor de piel o el modem podían declarar THROTTLE | ✅ zonas tipadas (soc/cpu/gpu) |
| **F29** | La "hybrid routing policy" (prefill → NPU EDEN) era **un string**: ningún módulo invoca EDEN/VNNI/NNAPI. El NPU era **decoración** | ✅ flag `npu_offload_implemented: false` explícito |
| **F30** | El crate era **binario puro**: `cargo test` no podía importar nada ⇒ **no había ninguna prueba automática posible** | ✅ `lib.rs` + `tests/audit_v13.rs` (21 tests) |
| **F31** | La ruta del HTML del cockpit estaba **hardcodeada al path de Android**: fuera del móvil el servidor devolvía un `<h1>` de **45 bytes** (la UI "no funcionaba" sin error visible) | `curl /` → 45 B vs 64 788 B tras el arreglo | ✅ resolución multi-ruta |

---

## 2. Lo que está arreglado y medido

### 2.1 El LLM local (SEERE v13.1 en Rust)

`rust-core/src/seere_v11.rs` — reescrito por dentro, misma interfaz pública más `SeereConfig`.

| Cambio | Antes | Después |
|---|---|---|
| Fingerprint del host | en el prompt del modelo | **sólo en el harness** (fundamento + ruta 0-token) |
| Lista OPS copiada | se ejecutaba como comandos | se descarta + turno de reparación |
| Cadena de acciones | 1 acción gigante | se parte en N acciones (`\| W …`, `\| C …`) |
| Pipeline `\| $ x` | 2 acciones | 1 comando (`\| x`) |
| `R: $ cmd` | respuesta inútil | **acción de última hora** + su salida es la respuesta |
| `R: A: cmd` | respuesta literal | se interpreta como acción |
| Acción truncada | continuación sin gramática | se descarta + reparación |
| Mensaje de reparación | podía ser la respuesta | se rechaza explícitamente |
| Cita `[SRC:n]` | no se comprobaba | **debe resolver** |
| Sin observaciones | números aceptados | **todo número es inventado** |
| Matching | subcadena | **límites de palabra** |
| Endpoint | hardcodeado | `SEERE_LLM` (host, puerto, TLS, ruta) |
| Reintentos / status / chunked | no / no / no | **3 intentos / 401-402-404-429 / sí** |
| Slot | fijado a 0 | el servidor elige (mejor prefijo KV) |
| Interfaz | 7 parámetros posicionales | **`SeereConfig`** |
| Prompt | 227 tok (ES) con opcodes RE | **207 tok** sin ellos |

**Resultado medido** (Qwen2.5-0.5B real, 12 preguntas que exigen datos):

```
v13.0 (con fingerprint)  routed= 6/12   sens= 3/12   grounded= 6/12
v13.1 (sin fingerprint)  routed= 8/12   sens= 8/12   grounded= 9/12
```

### 2.2 El catálogo de modelos

`rust-core/src/provider_catalog.rs` — **18 proveedores**, verificados contra la documentación oficial
(hoy): 2 offline, **2 zero-key** (sin cuenta ni key), **13 con plan gratuito sin tarjeta**.

| Proveedor | Modelo por defecto | Acceso |
|---|---|---|
| **local-seere** / **local-seere-vllm** | Qwen/SmolLM2 local | offline |
| **pollinations-free** | `openai` | **zero-key** |
| **ovhcloud-free** | `gpt-4o-mini` | **zero-key, anónimo** (2 req/min) |
| **google-ai-studio** | `gemini-3.8-flash` | free, sin tarjeta |
| **groq-free** | `openai/gpt-oss-120b` | free, 30 RPM / 1 000 RPD |
| **openrouter-free** | `openrouter/free` | free, 20 RPM / 50 RPD |
| **cloudflare-workers-ai** | `gpt-oss-120b` | free, 10 k neuronas/día |
| **zai-glm-free** | `glm-4.7-flash` | free (precio 0) |
| **mistral-free** | `mistral-small-latest` | free mode |
| **cohere-trial** | `command-a-03-2025` | trial, 1 000 llamadas/mes |
| **nvidia-nim** | `llama-3.3-nemotron-super-49b-v1` | créditos de build |
| **sambanova-free** | `Llama-4-Scout-17B-16E-Instruct` | free |
| **minimax-m3.1-flash** | `MiniMax-M3.1-Flash` | **1 M ctx, multimodal, `reasoning_effort`** |
| **minimax-m3** | `MiniMax-M3` | 1 M ctx, multimodal |
| cerebras / deepseek / anthropic | — | crédito o prepago |

Los **retirados** se exponen en `/api/providers` con su motivo, la UI los marca, y pedir uno por API
devuelve un error accionable en vez de fallar en silencio:

```
MiniMax-M2.7        → RETIRADO el 2026-11-18 -> usa MiniMax-M3.1-Flash
GitHub Models       → SERVICIO RETIRADO el 2026-07-30 -> usa OpenRouter o Groq
```

**Cómo conseguir las keys gratuitas** (todas sin tarjeta, salvo donde se indica):
Google AI Studio <https://aistudio.google.com/apikey> · Groq <https://console.groq.com/keys> ·
OpenRouter <https://openrouter.ai/keys> · Cloudflare <https://dash.cloudflare.com/> ·
Z.ai <https://z.ai/> · Mistral <https://console.mistral.ai/> · Cohere <https://dashboard.cohere.com/> ·
NVIDIA <https://build.nvidia.com/> · SambaNova <https://cloud.sambanova.ai/>.
Y **sin nada de eso**: `pollinations-free` y `ovhcloud-free` funcionan con la key vacía.

### 2.3 El cliente HTTPS

`call_openai_http` ya no hace `Command::new("curl")`: usa **ureq + rustls** (nativo, con CA embebido
→ funciona en Android sin nada instalado). Cero procesos, cero ficheros con la key en disco, y errores
que dicen lo que hay que hacer:

```
HTTP 401: (API key invalida o sin permisos)
HTTP 402: (sin saldo / plan agotado)
HTTP 404: (modelo inexistente o RETIRADO - mira el catalogo)
HTTP 429: (rate limit o cuota diaria agotada)
```

### 2.4 La energía: el daemon se adapta al móvil

`rust-core/src/power_policy.rs` — lee `/sys/class/power_supply`, `/sys/class/thermal` y
`/sys/devices/system/cpu/cpufreq`, y devuelve una política que el resto respeta:

| Política | Cuándo | turnos | local | trabajo pesado | tokens |
|---|---|---|---|---|---|
| `critical` | ≤15 % sin cargador, o batería ≥50 °C, o CPU ≥75 °C | 1 | sí | no | 256 |
| `saver` | ≤35 % sin cargador, o power-save del sistema | 2 | sí | no | 320 |
| `thermal` | CPU ≥62 °C aunque esté cargando | 2 | no | no | 384 |
| `performance` | con cargador y frío | 4 | no | sí | 448 |

Conectado a: `/api/chat` (recorta turnos y tokens), `/api/power` (nuevo), el comando `megadaemon-core power`,
y la cabecera del cockpit (`BAT 62% CHG`). **El daemon deja de quemar el móvil cuando no toca.**

### 2.5 La interfaz web

- **El cockpit ahora sirve la UI fuera del móvil** (antes: 45 bytes; ahora 64 788).
- El selector de modelo ya no propone un modelo **retirado** (`local-seere` por defecto).
- Los proveedores **retirados** salen como aviso ámbar en el panel, con el motivo y el sustituto.
- Los **18** proveedores se ven desde el primer arranque (antes: 2).
- Indicator de batería/carga/thermal en la cabecera, con la política activa.
- Cada proveedor trae su **enlace "GET KEY"** al alta gratuita correspondiente.

---

## 3. Lo que NO he hecho (y por qué) — hoja de ruta honesta

| Petición | Estado | Por qué / cómo seguir |
|---|---|---|
| **Java → Rust (Ghidra)** | **No portado** | Los dos `.java` son *scripts de Ghidra* (12.2, Java 25): sólo se ejecutan dentro de Ghidra, que arrastra un JDK de 300 MB. El camino sin Java ya existe en Rust (`cfg_lifter.rs`: CFG + lifting + McCabe; `rea_assistant.rs`: índice de 2,7 MB + transpilación C++) y es lo que usa el daemon por defecto. **Portar "el script" no es portar Ghidra**: un decompilador nativo ARM64/x86/MIPS de calidad Ghidra son ~20 k líneas. Siguiente paso razonable: (a) un *wrapper* Rust que invoque `analyzeHeadless` si Ghidra está instalado y caiga al lifting nativo si no; (b) implementar `--no-java` en `rea`. |
| **NPU real** | **Honestamente marcado como no implementado** | No existe backend EDEN/VNNI/NNAPI en llama.cpp para Exynos. Lo que SÍ acelera el LLM es **ARM dotprod (KleidiAI SDOT) en CPU**, que es lo que ya usa llama.cpp. El camino real: (a) compilar llama.cpp con `-DGGML_ARM=ON` + KleidiAI; (b) mover la VLA a **Mali-G78 con Vulkan compute** (el NPU no tiene un SDK público abierto); (c) usar el NPU para el preprocesado de visión (NMS, cuantización) vía NNAPI, que sí es alcanzable desde Android. |
| **VLA / visión** | **Auditado, no tocado** | `npu_cv.rs` hace luma + Sobel + delta L1 sobre el frame SHM: correcto y rápido, pero es CPU. Con `allow_heavy=false` (política saver) debería dejar de ejecutarse: **eso está pendiente de cablear**. |
| **Más providers nativos base** | **Parcial** | 18 proveedores de IA, pero los "nativos base" del RE (DiE/Binwalk/Tile/ImHex/MBA/CIRX/Rizin/Frida) ya están en Rust; lo que falta es exponerlos por MCP/HTTP uniforme. |
| **Convertir más Python a Rust** | **Estructura preparada** | `core/` (10 k líneas) es el que queda. El bloqueo real: `core/megadaemon.py` invoca a los binarios Rust por shell, así que el salto a lib+bin (`lib.rs`) que he hecho **desbloquea** que `core/` sea un crate más con `cargo test`. |
| **Ingeniería inversa Windows/Mac/DOS** | **No tocado** | `binwalk_die.rs` y `cfg_lifter.rs` ya cubren PE/Mach-O/MZ; lo que falta es cobertura de *formatos*, no de código. |

---

## 4. Cómo aplicarlo

```bash
# 1. Build limpio (verificado: 0 errores, 0 warnings nuevos)
cd rust-core && cargo build --release

# 2. Pruebas adversarias (21/21)
cargo test --test audit_v13

# 3. Self-test del agente local
./target/release/megadaemon-core seere-selftest

# 4. Ver el prompt exacto y su coste en tokens
./target/release/megadaemon-core seere-prompt es
./target/release/megadaemon-core seere-grammar

# 5. Estado de energía y política activa
./target/release/megadaemon-core power

# 6. Cockpit
./target/release/megadaemon-core cockpit 3000
#   -> /api/providers  (catálogo + retirados)
#   -> /api/power      (política)
```

**E2E reproducible del agente local** (necesita un llama-server con un 0.5B):

```bash
llama-server -m qwen2.5-0.5b-instruct-q4_k_m.gguf --port 8080 -c 8192 -t 2 --jinja
python3 rust-core/e2e_seere_rust.py     # A/B con/sin fingerprint
```

---

## 5. Lo que aprendí auditando (resumen)

1. **Un catálogo de modelos es código, y caduca.** El README del repo prometía GitHub Models y
   MiniMax M2.7; los dos dejaron de existir. El catálogo debe llevar **fecha de verificación** y
   probarse contra `/v1/models` en el arranque, no contra la memoria del autor.
2. **El "prompt echo" es un fallo de diseño del harness, no del modelo.** Cuatro modelos distintos
   copiaron la línea OPS. Ninguno fue un problema de "el modelo es tonto".
3. **El fingerprint del host en el prompt es veneno para un 0.5B.** El modelo lo lee como "datos que
   ya tengo" y deja de preguntar al shell. Es la mejor palanca medible que he encontrado: ×2,7 en
   corrección semántica, gratis.
4. **Un binario sin `lib.rs` no se puede testear.** Seis de los treinta hallazgos sólo salieron a la
   luz al poder escribir `cargo test`. El resto se cousió con `#[test]` en 21 casos que **vuelven a
   fallar si el defecto regresa**.
5. **Prometer en la telemetría lo que el código no hace es un bug.** La "hybrid routing policy" decía
   "prefill → NPU EDEN"; no había una sola llamada a EDEN. Ahora el flag lo dice.
6. **Un daemon en un móvil necesita una política de energía de primera clase**, no un
   `if battery < 20` escondido en el chat handler.

---

### Fuentes verificadas (nov-2026)

- [MiniMax — Model List (M2.5/M2.7 retirados el 2026-11-18; M3 y M3.1-Flash vigentes)](https://platform.minimax.io/docs/faq/models)
- [MiniMax — API Overview (base OpenAI `https://api.minimax.io/v1`, 1 M de contexto, `reasoning_effort`)](https://platform.minimax.io/docs/api-reference/api-overview)
- [BenchLeader — Free LLM API tiers (quién tiene plan gratuito hoy)](https://www.benchleader.com/free-llm-api)
- [free-llm.com — 83+ APIs gratuitas (límites reales por proveedor)](https://free-llm.com/)
- [Developers Digest — Free LLM APIs 2026 (GitHub Models retirado 30-jul-2026; HF credits fuera 15-oct-2026)](https://www.developersdigest.tech/models/free-api)
- [OpenRouter — Free LLM APIs comparadas](https://openrouter.ai/blog/tutorials/free-llm-apis-compared/)
- [llama.cpp — `tools/server/README.md` y fuentes de `server-context.cpp` (`n_past = get_common_prefix`)](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- crates.io — `ureq` 3.5.2 (rustls), `llama-cpp2` **retirado** → `llama-cpp-4` 0.12.0

---

## Addendum v13.2 — «NPU real»: de la telemetría decorativa al offload ejecutable

El hallazgo **F-28** de esta auditoría decía, con razón, que la *hybrid routing
policy* era una declaración sin código detrás. La v13.2 cierra ese hallazgo:

| | v13.0/v13.1 | v13.2 |
|---|---|---|
| `npu_offload_implemented` | `false` (el NPU sólo se observaba) | **`true`** — y con alcance exacto documentado |
| Camino al NPU | ninguno | **NNAPI INT8**: grafo conv3x3+relu+maxpool2x2 (`rust-core/src/npu_nnapi.rs`) |
| `hybrid_routing_policy` | un string | **`accelerator_policy::policy()`**: JSON tipado, un veredicto y un motivo por carga |
| Cargas prometidas | «YOLO/OCR/CLIP a 120 FPS en EDEN» | clasificadas en *implementado* / *posible* / *imposible* |
| Verificación | ninguna | 8 pruebas, incluida una que ejercita la ABI NNAPI completa contra una implementación independiente en C |
| UI | «NPU LIVE» si existía `/dev/vertex10` | «NPU ACCEL / GPU NNAPI / CPU FALLBACK» según el backend real |

**Cuatro bugs de ejecución** (no de compilación) aparecieron al testear el
camino FFI contra la NNAPI falsa: la entrada u8 se guardaba como i8, el pool
indexaba sin offset de canal, los escalares de padding/stride nunca se
enviaban, y el puntero a las formas del modelo quedaba colgante. Los cuatro
habrían producido detección de UI silenciosamente rota en el móvil.

Detalles, evidencia y bibliografía: **`docs/NPU_REAL.md`**.

### Hallazgos nuevos de esta auditoría (v13.2)

| ID | Hallazgo | Gravedad | Estado |
|---|---|---|---|
| F-32 | El detector de bordes leía la entrada `uint8` en un buffer `int8` (luma 230 → −26) | Alta | **Corregido** + test |
| F-33 | El max-pool indexaba el tensor de 2 canales sin el offset de canal | Alta | **Corregido** + test |
| F-34 | Los escalares int32 (padding/stride) no se enviaban a NNAPI: el driver no podía compilar el grafo | Alta | **Corregido** + test |
| F-35 | `dimensions` del `OperandType` era un puntero a un `Vec` temporal ya liberado (formas basura en el driver) | Alta | **Corregido** + test |
| F-36 | La librería NNAPI se duplicaba entre `npu_bridge.rs` y `npu_nnapi.rs` (dos fuentes de verdad) | Media | **Corregido**: una sola implementación |
| F-37 | La UI anunciaba «NPU LIVE» en función de `/dev/vertex10`, que no prueba ninguna ejecución | Media | **Corregido**: refleja el backend real |
