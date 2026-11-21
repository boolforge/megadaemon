# RE del Exynos 2100 (s5e9840) y por qué `ensure_live_shm()` entra en panic

> Encargo E3 del tablero (`ext-re-exynos`): volcado RE del SoC + análisis del panic
> de `shm.rs` + diff propuesto para `shm.rs` y `cockpit_server.rs` con su test.
> Nodo externo: `cloud-sandbox` (RE Exynos 2100). Todo lo de §2 está medido en el
> S21 Ultra (`SM-G998B`, ROM Dr.Ketan U08, kernel `5.4.242-28575149-abG998BXXSBGXED`).

## 1. El shm: qué es y por qué puede entrar en panic

`rust-core/src/shm.rs` es el **bus de frames zero-copy** entre el daemon y los *runners*
(objetivos `segacd` y compañía). El layout de la cabecera (32 B) que deserializa es:

| offset | tamaño | campo |
|---|---|---|
| 0 | 4 | `magic` (0x534E4154 = "SNAT") |
| 4 | 4 | `version` |
| 8 | 4 | `frame_counter` |
| 12 | 8 | `timestamp_us` |
| 20 | 2 | `width` |
| 22 | 2 | `height` |
| 24 | 2 | `bpp` |
| 26 | 2 | `format` |

El modo de fallo es trivial y **no está controlado**: todos esos campos se leen con
`try_into().unwrap()` (líneas 43-50) o con indexado directo (líneas 228-281). Si el segmento
compartido llega **más corto de lo esperado** —segmento recién creado,_frame truncado, o
`mmap` de un fichero que ha cambiado de tamaño— el `unwrap()`/el indexado **hace panic**.

## 2. El mecanismo del 502 (cadena completa)

```
ensure_live_shm()          -> panic (shm.rs: try_into().unwrap())
   ↓ (panic dentro de la seccion critica)
Mutex<GlobalCockpitState>  -> envenenado para siempre
   ↓
lock().unwrap()            -> panic en CADA petición posterior
   ↓
sin catch_unwind           -> el hilo de la conexión muere a mitad de la respuesta
   ↓
Cloudflare                 -> respuesta incompleta => 502
```

El watchdog (`deploy/99-megadaemon.sh`) sólo miraba `pgrep`: el proceso seguía vivo, luego
nunca reiniciaba. **El servidor quedaba "vivo" y negro**: exactamente el síntoma que se veía
desde fuera.

Nota del maestro: el corte observado en el móvil fue además por **el binario ausente durante la
instalación** (ventana del `mv megadaemon-core.new`) — el supervisor gastó los 4 reintentos y
el instalador reinició **sólo el daemon, no el supervisor**. El parche de robustez es
**preventivo** y ya está probado.

## 3. Diseño correcto

1. `ensure_live_shm()` **nunca** bajo el mutex global: crear/validar el shm **antes** de tomar
   el lock, y el lock sólo para mutar estado en memoria.
2. Ningún `lock().unwrap()`: usar `lock_ok()` (tolerante al veneno).
3. `catch_unwind` **por conexión**: la petición que falla responde **500**; no se corta el socket.
   (Un `catch_unwind` exterior que mueve el socket al closure corta *también* la petición que
   falla — el `502` sigue viendo un corte. Por eso el socket se pasa por `&mut`.)

## 4. Diff propuesto

Rama: `fix/robustness-20261120` (`rust-core/src/cockpit_server.rs`, `rust-core/src/lib.rs`,
`rust-core/src/peers.rs`, `rust-core/src/root_intel.rs`, `rust-core/src/sqlite_probe.rs`,
`deploy/99-megadaemon.sh`).

```diff
--- a/rust-core/src/lib.rs
+++ b/rust-core/src/lib.rs
+/// Toma el lock global sin que un panic previo lo deje envenenado.
+pub fn lock_ok<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
+    match m.lock() {
+        Ok(g) => g,
+        Err(poisoned) => poisoned.into_inner(),
+    }
+}
```

```diff
--- a/rust-core/src/cockpit_server.rs
+++ b/rust-core/src/cockpit_server.rs
-            let mut st = mutex.lock().unwrap();
-            ... st.runners / st.active_target ...
+            let valid_target = {
+                let st = crate::lock_ok(&mutex);
+                if st.runners.contains_key(&target) { target.clone() } else { st.active_target.clone() }
+            };
...
-                    ensure_live_shm(&valid_target);   // <- DENTRO del lock: puede envenenar el mutex
+                    // ensure_live_shm() FUERA del lock: es el unico punto de este
+                    // bloque que puede hacer panic (shm.rs) y no puede hacerlo con
+                    // el mutex global cogido.
+                    ensure_live_shm(&valid_target);
+                    let runners = { let mut st = crate::lock_ok(&mutex); ...; st.runners.clone() };
```

```diff
--- a/rust-core/src/cockpit_server.rs   (por conexión)
+++ b/rust-core/src/cockpit_server.rs
-fn handle_client(mut stream: TcpStream) { ... }
+fn handle_client(mut stream: TcpStream) {
+    // wrapper: el panic se convierte en 500, no en conexión cortada
+    ...
+fn handle_client_inner(stream: &mut TcpStream) { ... }   // 122 usos de &mut stream
```

Watchdog de **responsividad** (`deploy/99-megadaemon.sh`): pregunta a `/health` con el
`python3` de Termux; **3 fallos ⇒ reinicia el daemon**, en vez de confiar sólo en `pgrep`.

## 5. El test que demuestra el fix

`rust-core/tests/cockpit_panic_resilience.rs` (nuevo, en la misma rama):

| test | qué fija |
|---|---|
| `lock_ok_tolera_un_mutex_envenenado` | tras envenenar el mutex, `lock_ok` sigue devolviendo el guard y permite operar |
| `un_panic_no_tumba_el_servidor` | la petición que provoca el panic responde **500** (no conexión cortada) y las **5 siguientes** responden **200** |

Demostración reproducible fuera del repo (`tools/panic_proof.rs`, mismo patrón):
- **sin parche**: tras un panic, las siguientes peticiones **no responden** (servidor vivo pero negro);
- **con parche**: siguientes = 200, 200.

Cifras del parche (`tools/bench_daemon.py`): `/health` con 200 peticiones y 32 concurrentes →
p50 100,2 ms · p95 211,6 ms · **0 errores** · 48,5 rps (binario anterior: p50 105,4 · p95 189,1 ·
48,9 rps ⇒ sin regresión). `/peers` (nuevo): 192/192 sin errores, p95 165,1 ms, 65,4 rps.

## 6. RE del Exynos 2100 — resumen de lo medido en el dispositivo

Todo verificado contra el móvil, no contra el ROM:

- **NPU/EDEN**: `/dev/vertex10` es `crw-r--r-- 1 system system 82,10` ⇒ **0644, legible por
  cualquier app**. EDEN **v1.6.23** en el móvil (el ROM da v1.6.20: builds distintos).
  Interfaz **VS4L** (9 ioctls `_IO('V',…)`/`_IOW('V',…)`, no el ioctl clásico) verificada sobre
  el binario del dispositivo (sha256 `7c0041ab…`). El NPU **ya está en producción**: conv3x3
  INT8 (Sobel) + relu + maxpool2x2 sobre frame SHM vía NNAPI; `megadaemon_npu_cv` mide
  telemetría en vivo (latencia CV 101,71 ms).
- **LLM en el NPU: imposible** (medido): NNAPI no tiene ops de atención ni formas dinámicas y
  llama.cpp no tiene backend Exynos; Vulkan/Mali-G78 mide **2× más lento** que la CPU en decode.
- **TEE**: el mundo seguro es **TEEGris** (`tzsw.img` con `TEEGRIS ver`), no GreenTea.
  `/dev/rpmb` **no existe** como nodo (el RPMB se alcanza por el TEE) —corrige un dato previo—.
  `/dev/tzdev`, `/dev/tziwsock`, `/dev/tziwshmem` en **0666**: cualquier app entra en el mundo seguro.
- **Boot chain**: el SBL implementa **AVB encadenado** (`chained vbmeta image has non-zero flags`,
  `DEBUG_VBMETA`) y **FMM Lock** con unlock gateado por `manid`. El `boot.img` del VN-ROM va
  reempaquetado con `androidboot.selinux=permissive` ⇒ **no es stock**.
- **Vendor**: **134 ioctls en 17 librerías**; 6 drivers con contrato confirmado
  (`libbauthserver.so` letras `g`/`k` de Game Framework-Knox, `libeden_ud_npu.so` `V`,
  `libexynosgscaler.so` y `libexynosv4l2.so` `V`, `libsbwcwrapper.so` `F`/`V`, `libese-grdg.so` `S`).
  Hallazgo estructural: **Samsung reutiliza la letra `'V'`** para `VIDIOC_*` estándar y para sus
  ioctls privados (`VS4L_VERTEXIOC_*`); confirmado en hardware porque `/proc/devices` declara
  major 82 = `vision4linux`.
- **340+ binarios** decompilados a C con Ghidra + **3 oráculos** (unicorn, Z3, angr).
  Veredicto de ofuscación: **no hay ofuscación intencional** en el corpus (2,32 % ARM32 /
  1,37 % A64 de veredictos; los histogramas históricos 3,81 % y 4,18 % quedan **retirados**).
- **El móvil NO es UN1CA**: corre `G998BXXSBGXED_Dr.Ketan_U08_XDA` (OneUI 6.0, Android 14,
  patch 2024-06-01) con 21 módulos Magisk (playintegrityfix 37, tricky_store 1.4.1,
  luckypatcher_zygisk 12.10.6, flagsecurepatcher r17, cromite-webview 153…).

Detalle completo: `docs/ANDROID_EXYNOS2100_FINDINGS.md` (maestro) y
`docs/P1_NPU_EDEN.md`, `P2_TEE_SECURE.md`, `P3_BOOT_CHAIN.md`, `P4_VENDOR_IOCTLS.md`,
`P5_ROOT_DETECTION.md` (nodo externo, repo `port/exynos2100`).

## 7. Regla legal

El C decompilado de proprietary **no se versiona**: vive sólo en `/workspace/output/mass_decomp`.
`scripts/check_no_proprietary.py` queda verde.
