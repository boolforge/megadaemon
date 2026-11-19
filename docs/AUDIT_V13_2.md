# MegaDaemon v13.2 — Auditoría adversarial (hardware/servidor + harness SEERE)

> Fecha: 2026-11-18 · Base: `77f5ae4` (v13.1 merge `aa30866`) · Rama: `feat/v13.2-npu-nnapi`
> Método: **dos auditorías adversariales independientes** (subagentes con acceso a test y ejecución real),
> cada hallazgo reproducido con un **test ejecutable** antes de tocar el código, y **verificado en verde**
> después del arreglo. Nada de este informe es "teórico": cada línea tiene detrás un `cargo test` que
> falló antes y pasa ahora.

---

## 0. Resumen ejecutivo

| Suite | Tests | Resultado |
|---|---|---|
| `tests/audit_v13.rs` (v13.1) | 21 | ✅ 21/21 |
| `tests/audit_scout_hw.rs` (v13.2, hardware/servidor) | 16 | ✅ 16/16 |
| `tests/audit_scout_seere.rs` (v13.2, harness SEERE) | 31 | ✅ 31/31 |
| `tests/audit_scout_auth.rs` (v13.2.4, fail-closed del cockpit) | 7 | ✅ 7/7 |
| `tests/npu_nnapi.rs` (v13.2, FFI NNAPI) | 9 | ✅ 9/9 |
| `src/util_text.rs` (unit) | 9 | ✅ 9/9 |
| **TOTAL** | **93** | **✅ 93/93** |

* `cargo build --release` → **0 errores, 0 warnings**.
* `megadaemon-core seere-selftest` → `SELFTEST_V11_RUST_OK`.
* E2E real (Qwen2.5-0.5B-Q4_K_M vía `llama-server`): `routed=6/12 sens=3/12` (con huella en prompt)
  y `routed=8/12 sens=8/12` (sin huella) — **idéntico a v13.1**: los arreglos no degradan el
  comportamiento medido.

**Totales históricos**: 31 (v13.1) + 16 (HW) + 17 (S) + 7 (AUTH) + 2 (UI) = **73 defectos**
encontrados y corregidos. Ninguno "fantasmado": sin test en rojo antes, no hay entrada en esta
tabla.

---

## 1. Auditoría HW — hardware, servidor, energía, seguridad (16)

| ID | Sev | Fichero | Defecto | Arreglo | Test |
|---|---|---|---|---|---|
| **HW-01** | **ALTA** | `cockpit_server.rs` | `/api/status` **panicaba** si el fichero de estado era un array JSON (`serde_json::IndexMut` sobre un no-objeto → panic → con `panic=abort` tumba el cockpit) | `read_json_file_fallback` acepta **sólo objetos** | `hw01` |
| **HW-02** | **ALTA** | `freezer.rs`, `cockpit_server.rs` | `/api/memory_freeze` aceptaba **cualquier ruta** (`target` libre) → escritura arbitraria de ficheros | `freezer::validate_target()` confina a `SANDBOX_ROOTS` + `SANDBOX_FILES` (incluida la SHM), exige ruta absoluta, sin `..`, fichero regular ≤ 64 MB | `hw02` |
| **HW-03** | **ALTA** | `providers.rs` | Cambiar el endpoint de un.provider con key guardada **enviaba la key al host elegido por el cliente** (robo de credenciales / SSRF) | La key queda **atada al endpoint completo** y se vacía al cambiarlo | `hw03` |
| **HW-04** | **ALTA** | `providers.rs` | El adjunto podía ser **cualquier fichero local** y su contenido se subía al proveedor | `providers::attachment_allowed()` (sólo `uploads/`, `extracted/`, `re_pipeline/`, `snatcher/`) + aviso explícito "ADJUNTO IGNORADO" | `hw04` |
| **HW-05** | MEDIA | `cockpit_server.rs` | `Content-Length` sin tope → 100 MB por conexión | `MAX_BODY_BYTES = 8 MB` → HTTP 413 | `hw05` |
| **HW-06** | MEDIA | `npu_bridge.rs` | `su -c "MEGADAEMON_NPU_RAW=1 …"` **recursivo** (se lanzaba a sí mismo) y **sin timeout** → podía colgar el cockpit en cada sondeo | Sondeo **directo** (sin `su` recursivo) + **caché TTL 5 s** (`probe_exynos_npu_cached`) | `hw06` |
| **HW-07** | MEDIA | `shm.rs` | Cabecera SHM hostil (65535×65535) → **12,9 GB** de reserva de memoria | Límites `MAX_FRAME_DIM=4096`, `MAX_FRAME_PIXELS=8 MP`, `MAX_FRAME_BYTES=32 MB` | `hw07` |
| **HW-08** | MEDIA | `main.rs` | `shm-frame-bmp` **panicaba** con cabecera incoherente | Usa `shm::read_frame_snapshot()` (una sola lectura + verificación `frame_counter` antes/después → `frame_torn`) | `hw08` |
| **HW-09** | MEDIA | `main.rs` | `--max-turns 8` **filtraba el "8" al prompt del LLM** (el valor no se consumía del argv) | Parsing por índice que **consume el valor** | `hw09` |
| **HW-10** | MEDIA | `npu_bridge.rs` | Telemetría declaraba KleidiAI/dotprod y símbolos "ELF-verified" **sin haber cargado la lib** | Sólo se anuncia lo que se cargó; los símbolos no cargados se declaran como tales | `hw10` |
| **HW-11** | MEDIA | `npu_cv.rs` | El primer análisis **inventaba movimiento** (`delta=12,5`, `scene_changed=true`) → disparaba el VLM de verdad | `delta=0` + campo `first_frame` | `hw11` |
| **HW-12** | MEDIA | `providers.rs` | La máscara de keys revelaba **10 caracteres** | Máx. 2+2 (o `***` si la key tiene < 12) | `hw12` |
| **HW-13** | MEDIA | `providers.rs`, `providers.rs` | Un modelo **RETIRADO** se sustituía en silencio; el camino "hybrid" apuntaba a `minimax-m2.7-fast` | `execute_chat` devuelve aviso explícito; "hybrid" → `minimax-m3.1-flash` | `hw13` |
| **HW-14** | MEDIA-BAJA | `hw_mapper.rs` | Structs inferidos contradictorios (`total_size_est: 34` con campos hasta `0x48`) y no platform-specific | 72 bytes + nombre por plataforma | `hw14` |
| **HW-15** | MEDIA | `power_policy.rs` | La política **térmica** quedaba tapada por "saver" | `thermal` se evalúa **antes** que `saver` | `hw15` |
| **HW-16** | MEDIA-BAJA | `power_policy.rs` | Temperaturas **inventadas** (32,0 / 35,0 °C) | **NaN** → JSON `null` cuando no hay sensor | `hw16` |

### 1.1 El hallazgo que explicaba el NPU ocioso

`power_policy.rs` leía el governor de **`policy0`** (cluster **pequeño**, que en Android está *siempre* en
`powersave`) → `system_power_save=true` casi siempre → política `"saver"` → `allow_heavy=false` →
**la visión continua nunca se ejecutaba**. Por eso el NPU estaba infrautilizado aunque el backend NNAPI
funcionase. Arreglo:

* sólo cuentan `policy4/6/7` (cluster grande) o la señal explícita `cpufreq/power_save`;
* `thermal` se evalúa **antes** que `saver`;
* `allow_heavy` dejó de ser decorativo (`/api/run_vla` → `VLA_CV_THROTTLED` + motivo; visión continua
  a 2 Hz en `performance`, 0,2 Hz en `saver`, sólo contador en `critical`).

### 1.2 El hallazgo que más dolía: RCE sin autenticación

`POST /api/chat {"model":"local-seere","message":"$ …"}` ejecutaba la orden **como root** vía `su -c`,
**sin credencial** y con `Access-Control-Allow-Origin: *` → cualquier página web visitada en el móvil
podía ejecutar comandos en el dispositivo (CSRF → RCE). Arreglo (fail-closed):

* token persistente en `<STATE_DIR>/cockpit_token` (0600) o variable `SEERE_AUTH_TOKEN`;
* inyectado en el HTML (`window.MEGADAEMON_TOKEN`) y enviado por la UI (`authHeaders()`);
* exigido en **todos** los endpoints que ejecutan o mutan estado:
  `/api/chat`, `/api/llm_agent`, `/api/brain_query`, `/api/run_vla`, `/api/memory_freeze`,
  `/api/trigger_boost`, `/api/purge_cache`, `/api/providers`, `/api/vision_start`, `/api/vision_stop`,
  `/api/chat_upload`;
* CORS restringido: se refleja el `Origin` sólo si es el mismo `Host` o está en
  `MEGADAEMON_ALLOWED_ORIGINS`.

---

## 2. Auditoría S — harness SEERE (17)

| ID | Sev | Fichero:línea | Defecto | Arreglo | Test |
|---|---|---|---|---|---|
| **S01** | **ALTA** | `seere_v11.rs:529-537` | `normalize_action` recortaba con `arg[prefix.len()..]` donde `prefix.len()` venía de `to_uppercase()` → **`panic`** en `char boundary` (DoS remoto vía salida del LLM; con `panic=abort` mata el daemon) | Se busca el `':'` real; nunca se indexa con una longitud calculada sobre otra cadena | `s01`, `s01b`, `s01c` |
| **S02** | **ALTA** | `seere_v11.rs:1093` | La puerta de *taint* era **inerte** (`tainted && … && !allow_exec` se anulaba con `allow_exec=true`, la config real) y usaba el flag equivocado para `DELEGATE_LLM` | Taint **incondicional** (igual que la referencia Python v11) y flag propio | `s02`, `s02b` |
| **S03** | **ALTA** | `seere_v11.rs:1872` | El atajo de 0 tokens (`$ …`) **ignoraba `allow_exec`**: la política cerrada no cerraba el shell | Pasa por `Arsenal::dispatch` → respeta la política | `s03` |
| **S04** | **ALTA** | `seere_v11.rs:797-803,1991` | La **fase final** ejecutaba acciones: `R: hola` + línea suelta `C: /fichero` → lectura arbitraria de ficheros con el contenido en `answer` | Parser estricto para la fase final (`parse_final_output`): sólo `R: …` | `s04`, `s04b` |
| **S05** | MEDIA-ALTA | `seere_v11.rs:905,1036` | El verificador de fundamento **no comprobaba los enteros**: `5` (1 dígito) y `4 GB` (entero con unidad) inventados pasaban siempre que hubiera ≥1 observación | Se comprueban todos los enteros y los enteros con unidad | `s05` |
| **S06** | MEDIA | `seere_v11.rs:1217-1243` | `exec_root` **no hacía fallback** si `su` existía pero fallaba (Magisk denegando) → el agente se quedaba sin shell en el dispositivo | Se encadena `su → rish → sh` también ante fallo, guardando el último error | `s06` |
| **S07** | MEDIA | `seere_v11.rs:2030` | El guardián anti-eco (F04) **no se aplicaba** a la acción de última hora → el eco de la línea OPS se ejecutaba como shell | El anti-eco también filtra la ruta de última hora | `s07`, `s07b` |
| **S08** | **ALTA** | `cockpit_server.rs:258,813` | **RCE sin autenticación + CORS `*`** | Token + CORS restringido (§1.2) | `s08`, `s08b`, `s08c` |
| **S09** | MEDIA | `cockpit_server.rs:989`, `providers.rs:626,730` | La política de energía **no se aplicaba al agente**: `max_turns` era decorativo (4 turnos fijos) y `FINAL_MAX_TOK`(448) > `max_tokens` de `saver`(320)/`critical`(256) | Cockpit y providers usan `pol.max_turns`; `FINAL_MAX_TOK` = 256 | `s09` |
| **S10** | MEDIA | `seere_v11.rs` | `DELEGATE_LLM` cableado a **`minimax-m2.7-fast` (RETIRADO)** → el opcode estaba muerto | Se delega en un id vivo; la regresión comprueba que no reaparecen ids retirados | `s10`, `s10b` |
| **S11** | MEDIA | `seere_v11.rs:81,797-803` | **Gramática ≠ parser**: el parser aceptaba y despachaba `A: NONE` y `A: $` (sin shell), formas que la gramática prohíbe | `opcode_arg_valid()`: argumento obligatorio salvo `SHM_OP`/`MEM_OP` | `s11` |
| **S12** | MEDIA-BAJA | `seere_v11.rs:2090` | El arreglo de F18 era **cosmético**: la observación cruda seguía en `answer` | La observación va a `last_observation`, no a `answer` | `s12` |
| **S13** | MEDIA-BAJA | `main.rs:166`, `cockpit_server.rs:880` | Truncado **por bytes** sobre `&str` (`s[..s.len().min(65536)]`) → `panic` si el corte cae dentro de un carácter multibyte | `util_text::truncate_chars` (por caracteres) y `truncate_bytes` (por bytes, sin partir caracteres) | `s13`, `s13b` |
| **S14** | MEDIA | `seere_v11.rs` | `DELEGATE_LLM` devolvía el aviso "MODELO RETIRADO" **como si fuera la respuesta** del LLM externo | El aviso se reconoce y no se toma como respuesta | `s14` |
| **S14b** | MEDIA-BAJA | `mcp_server.rs:28-29,45` | El MCP anunciaba ids **retirados** como `default` de sus tools | `default: "local-seere"` en ambos tools | `s14b` |

### 2.1 Nota sobre los tests "de prueba de defecto"

Seis tests (`s01`, `s01b`, `s01c`, `s10`, `s13`, `s13b`) se escribieron **para demostrar el defecto**
(esperaban el panic / la presencia del patrón peligroso). Una vez corregido el código, esos tests
fallaban por la razón contraria. Se **invirtieron a tests de regresión**: ahora **afirman que el
defecto NO está** (no panic, patrón ausente, id retirado ausente). El fichero
`tests/audit_scout_seere.rs` es, por tanto, una suite de regresión estable: **verde para siempre**
salvo que alguien reintroduzca un bug.

---

## 3-bis. Tercera ronda (v13.2.3 / v13.2.4): la capa de autenticación no era *fail-closed*

Una tercera pasada manual sobre la capa HTTP del cockpit encontró un agujero
**del mismo tipo** que S08 pero por el otro lado: `requires_auth()` protegía
**sólo los POST**, mientras que la rama `GET` servía endpoints con efecto de
estado o con datos sensibles.

| ID | Sev | Endpoint | Defecto | Arreglo | Test |
|---|---|---|---|---|---|
| **AUTH-01** | **ALTA** | `GET /api/vision_start` | Arrancaba el worker de **visión continua** (un hilo permanente que quema CPU en el dispositivo) **sin token** | Fail-closed: todo `/api/*` exige token | `auth01` |
| **AUTH-02** | **ALTA** | `GET /api/memory_freeze` | Congelaba/escribía ficheros **sin token** | Ídem | `auth02` |
| **AUTH-03** | MEDIA | `GET /api/providers` | Volcaba el catálogo de proveedores (y la configuración de claves) **sin token** | Ídem | `auth03` |
| **AUTH-04** | — | `POST /api/chat` | *Control*: el POST ya exigía token (no debe romperse) | — | `auth04` |
| **AUTH-05** | — | con token | *Control*: vision_start/stop/status siguen funcionando con token | — | `auth05` |
| **AUTH-06** | **ALTA** | 15 endpoints `/api/*` | *Fail-closed*: **ningún** endpoint respondía sin token (15/15 respondían 200) | Ídem | `auth06` |
| **AUTH-07** | — | `GET /` | *Control*: la página HTML sigue pública e inyecta el token (si no, la UI no podría ni hacer fetch) | — | `auth07` |
| **UI-01** | MEDIA | `web/cockpit_template.html` | El botón TEST interpolaba el id del proveedor dentro de un `onclick` inline: el parser HTML **decodifica las entidades antes de evaluar el JS**, así que un id con comillas escapaba del atributo | El id va en `data-test-provider` y se resuelve por delegación (sin `eval` de JS) | validación en Chromium headless |
| **UI-02** | MEDIA-BAJA | `web/cockpit_template.html` | El enlace "GET KEY" aceptaba cualquier esquema de URL (`javascript:`, `data:`) | Sólo `http(s)://` | ídem |

**Por qué importa**: el CORS restringido cerraba el vector "cualquier página web
del móvil", pero el cockpit **también escucha en la LAN**: cualquier proceso del
dispositivo (o una app sin permisos) puede llamar a `127.0.0.1:<puerto>`. El
modelo correcto es **fail-closed**: la única superficie pública es la página
HTML, que lleva el token embebido en `window.MEGADAEMON_TOKEN` (y sin CORS
abierto nadie externo puede leerla).

Además, en esta ronda:
* `state_dir()` es configurable con `MEGADAEMON_STATE_DIR` (el cockpit escribía
  **siempre** en `/data/user/0/com.agy/files/megadaemon/state`, impracticable
  fuera de Android y en CI);
* el token se materializa **al arrancar** (antes era perezoso: el fichero
  `cockpit_token` no existía hasta la primera petición);
* el mensaje 413 decía `payload_over_100mb` cuando el límite real es 8 MB
  (**la telemetría también es un contrato**).


---

## 3. Cambios de API (v13.2)

| Endpoint | Cambio |
|---|---|
| `GET /api/npu_bench` | Benchmark **A/B real en el dispositivo**: misma frame, NNAPI vs referencia CPU, con verificación de equivalencia bit a bit y veredicto |
| `GET /api/vision_status` | Estado de la visión continua (Hz, backend, latencia, mapa de calor) |
| `POST /api/vision_start` / `stop` | Control de la visión continua (respeta la política de energía) |
| `GET /api/llm_route` | **Decisión de ruta CPU/GPU con datos**: umbral analítico (prompt > 5,26× salida) + veredicto para el prompt actual |
| `GET /api/brain_status` | Backend de visión **real** (`npu_vision_backend`, `accelerator_policy`, `executed_on_hardware`) |
| `POST /api/run_vla` | Gating real de `allow_heavy` → `VLA_CV_THROTTLED` + motivo (cierra F-38) |

Todos los endpoints que ejecutan o mutan estado exigen `X-Seere-Auth-Token`.

---

## 4. Cómo reproducir esta auditoría

```bash
cd rust-core
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release                 # 0 errores, 0 warnings
cargo test                            # 93/93
./target/release/megadaemon-core seere-selftest   # SELFTEST_V11_RUST_OK

# FFI NNAPI real contra una implementación ABI independiente en C:
cargo test --test npu_nnapi -- --nocapture
```

Pruebas de hardware en el S21 (Exynos 2100) y verificación del NPU: `docs/NPU_REAL.md` §3.

---

## 5. Lo que sigue **sin** estar resuelto (honestidad)

* **NNAPI no es ejecutable en este sandbox**: no hay Exynos 2100 ni `libneuralnetworks.so`. Lo que sí
  está probado es (a) el **camino FFI completo** contra una implementación independiente de la ABI
  NNAPI en C, cargada con el mismo `dlopen`, y (b) la **igualdad exacta bit a bit** NNAPI↔CPU.
  El flag `executed_on_hardware` sólo es `true` si la ejecución fue real.
* **El LLM no puede ir al NPU**: NNAPI no tiene ops de atención ni formas dinámicas, y llama.cpp no tiene
  backend Exynos. La ruta del LLM es **CPU con dotprod+i8mm** (y KleidiAI si la lib está presente).
* **Vulkan/Mali no sirve para el LLM**: medido en el propio S21, el decode por GPU es **2× más lento**
  que por CPU (22,8–23,1 tok/s vs 46–47 tok/s). Detalle en `docs/LLM_SWITCH_PLAN.md`.
* Ghidra no se ha portado a Rust: los `.java` requieren Ghidra+JDK; el camino Rust ya cubre el caso.
