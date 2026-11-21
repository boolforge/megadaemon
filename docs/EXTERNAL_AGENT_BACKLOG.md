# BACKLOG CONCRETO para el agente EXTERNO (nodo RE externo)

> Esto es para TI, agente externo, que entraste por la instancia maestra (Samsung).
> No te quedes sin tarea: esta lista es tu cola. Cuando termines un punto, pasa al
> siguiente y deja **una línea** al final de `docs/COORDINATION.md` con:
> **qué hiciste / qué tocaste / qué bloquea**. Si te atascas, escribe en el tablón
> y sigue con el siguiente punto (no pares por un bloqueo: hay 12 puntos).

Contexto mínimo para no perderte:
- Tu instancia es la **MAESTRA `primary`** (el móvil Samsung, `<IP_DEL_MOVIL>`).
- Tú eres **`worker`**. La maestra decide; tú aportas datos y parches.
- **Conexión por túnel** (immune a bloqueo por IP):
  `https://TUNEL.trycloudflare.com` · MCP: `POST /mcp` (JSON-RPC 2.0, **no** `/sse`).
  Cabeceras: `X-API-Key: <mdaemon_sk_live_…>` (la tienes) y `X-Seere-Auth-Token: <cockpit_token>` para POST/PUT.
- **No hay binario `adb` en el móvil**: la flota habla ADB en Rust puro. Tool MCP `adb`:
  `status | shell | devices | keyinfo | push | pull` (ya integrada en `main`).

---

## P0 — El 502 del maestro (lo estás haciendo tú, con estos datos)

Tu hipótesis (mutex global envenenado por un panic → `lock().unwrap()` vuelve a entrar en
panic → respuesta cortada → 502) es correcta, y **tienes el panic exacto medido** en la
auditoría del hardware (`rust-core/tests/audit_scout_hw.rs`, 16 tests, muchos ya en verde):

| ID | Qué pasa | Mensaje real | Dónde |
|---|---|---|---|
| **HW-01** | `GET /api/status` entra en panic y **mata el daemon** | `cannot access key "cockpit_engine" in JSON array` · en release: `SERVER EXIT CODE=134` (SIGABRT, `panic="abort"`) | `cockpit_server.rs` (parseo del JSON de estado) |
| **HW-07** | La cabecera SHM declara `65535x65535` → pide **12.884.508.675 B** | `memory allocation of 12884508675 bytes failed` → SIGABRT | `rust-core/src/shm.rs` (**lane mío**: avisa antes de tocarlo y coordínate) |
| **HW-08** | Índice fuera de rango al copiar el frame | `range end index 3032824 out of range for slice of length 1048576` | `rust-core/src/main.rs:452` |

**Lo que se pide (P0)**, con test de regresión obligatorio:
1. `catch_unwind` por conexión (una petición panic no tumba el daemon).
2. Mutex global **a prueba de veneno** (o `lock()` con `unwrap_or_else(|e| e.into_inner())`).
3. `ensure_live_shm()` **fuera** del lock global (es una llamada lenta y puede entrar en panic).
4. Validar las dimensiones de la cabecera SHM **antes** de mapear (nada de reservas de 12 GB).
5. **Watchdog de responsividad**, no de `pgrep`: si `/health` no responde en X s → reinicio.
6. Test: reproducir el panic de HW-01 y **demostrar que el daemon sigue sirviendo** después.
   Es el DoD del plan: test + evidencia (salida real pegada).

## P1 — Volcado completo de datos (la Fase 1 que te pidió el usuario)

El maestro está vacío de lo que tú sabes. Vuelca, con sha256 y resumen, al almacén común:
APK/DEX de cada app (package, versión, firma, activities/receivers/providers, permisos) ·
inventario de hardware (SoC/Exynos, NPU, sensores, particiones, DRAM) · SQLite de apps ·
logs, binarios y configs en caché. Devuelve el **índice** de lo volcado.

## P2 — ADB por la tool MCP (sin binario, sin IP)

- `adb status` → ¿responde `adbd`? ¿banner? ¿clave de host cargada?
- `adb devices` → inventario de dispositivos alcanzables por el maestro.
- `adb shell "<orden>"` → INTROSPECCIÓN del propio móvil (root): `getprop`, `pm list packages -f`,
  `ls /data/app`, `dumpsys meminfo`, `cat /proc/cpuinfo`.
- `adb keyinfo` → qué claves de host están registradas (`mavis@samsung`, `mavis@fleet`, `mavis@user-pc`).
- `adb pull` de artefactos interesting (APKs de `/data/app`) → volcarlos al almacén común.
- **Ojo**: `adbd` exige firma RSA de clave de host; la tool debe **informar** de eso
  (fail-closed), nunca abrir un shell sin firmar.

## P3 — Build ARM64 (el móvil sigue con v13.1)

El móvil corre **v13.1**: sólo escucha en `127.0.0.1:9090` y su gate de API key devuelve
401 a `/` (por eso la UI.necesitaba un puente). El binario nuevo (alias 3000/3030/8090 +
`/` exento del gate) **elimina el puente**. Compila ARM64 con toolchain propia, `CARGO_TARGET_DIR`
en disco local (el workspace es NFS: compilar dentro tarda 8+ min y da fallos falsos), `-j 1`
(la caja tiene 3 GB de RAM y 2 CPUs) y despliega con **verificación y reversión** si el
daemon no levanta.

## P4 — Saturación del móvil (medido, no es culpa de MegaDaemon)

En el Samsung: **10,2 GB usados de 10,5 GB, 3,3 GB de swap, 78 % iow, load 15,9**. Lo comen
las apps de Android (system_server 535 MB, servicios Samsung 511 MB, Chrome 462 MB, systemui
382 MB, teclado 302 MB, WhatsApp 282 MB, GMS 550 MB, DeepSeek 255 MB). **MegaDaemon: 8,9 MB RSS,
4 hilos, CPU idle**. Acción: congelar/matar apps pesadas en segundo plano, bajar el swap, y
**no** añadir procesos ni polling al móvil (el daemon se autoprotege: 1 conexión, sin bucles).

## P5 — Optimización del daemon (sin quitar capacidades)

Cuellos ya medidos y con dueño:
- `power_policy.rs` (**mío**): el polling de la UI releía ~15 ficheros de sysfs + `read_dir`
  de `/sys/class/thermal` en cada llamada → **caché TTL de 1,5 s** ya integrada y verificada.
- `npu_cv.rs` (**mío**): `vec![0u8; pixels]` por frame (2 MB en 1080p) → reutilizar buffer.
- `shm.rs` (**mío**): reservar la cabecera validada, no 12 GB.
- Cockpit (**agente de cockpit**): JSON grande por petición, `Content-Length: 100MB` sin tope
  (HW-05: +102.516 KiB de RSS), `/api/memory_freeze` con escritura arbitraria (HW-02),
  SSRF hacia un host elegido por el cliente (HW-03), fuga de fichero local al proveedor (HW-04).

## P6 — Hallazgos de la auditoría que puedes cerrar

De `rust-core/tests/audit_scout_hw.rs` (16, con salida real): HW-05 tope de `Content-Length` ·
HW-06 `su -c` sin timeout ·HW-09 `--max-turns` colado en la query · HW-10 telemetría falsa
(`kleidiai_dotprod_active=true` con `.so` no cargable) · HW-11 `delta=12.5 scene_changed=true`
en el primer frame · HW-12 máscara de 11 píxeles con 10 visibles · HW-13 modelo RETIRADO
sustituido en silencio con `fallback_used=false` · HW-14 `total_size_est=34` con campos hasta 72 ·
HW-15 política `saver` con CPU a 70 °C.

## P7 — Protocolo con el enjambre (para que no te aburras)

- **Tablón**: `docs/COORDINATION.md` → una línea al final, siempre: *qué hice / qué toqué / qué bloquea*.
- **Principal**: sesión `6d7ffc8f-6d78-4cc8-81ca-ae6e5e714b85` (es quien reparte y decide).
- **Bus de coordinación**: `9cb64913-f5e8-4678-b648-645b09c47161` (para tareas cruzadas).
- **Malla/transporte**: `23c1e5eb-8f68-48b8-8d46-6ef2b2d1f54b` (coordinación.rs = estado, mesh.rs = transporte).
- Reglas del repo: `git add <tu ruta exacta>` (nunca `-A`), un `CARGO_TARGET_DIR` propio,
  y **nunca** toques ficheros de otro módulo sin avisar antes al principal.
- Si un punto se bloquea: sáltalo, escribe la línea al tablón y sigue con el siguiente.
  **No pares sin escribir**: un bloqueo sin(reportar) = hueco en el trabajo de todos.
