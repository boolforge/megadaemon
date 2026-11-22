# Backlog de Antigravity (recuperado del S21 Ultra) — 2026-11-22

> **Qué es esto**: el backlog real del cliente Antigravity (`com.agy`) del S21
> Ultra, **leído del dispositivo por root** el 2026-11-22 (01:00–01:12 CET),
> con el estado que tenía **de verdad** y el **diagnóstico del corte** del
> micro-daemon que lo dejó colgado.
>
> **Todo lo de aquí está MEDIDO** en el móvil salvo lo que va marcado como
> `NO VERIFICADO`. Las horas son del reloj del dispositivo (CET, `date` del
> propio móvil). Los `epoch` se dan entre paréntesis para poder contrastarlos
> sin depender del huso.
>
> **Lane**: agente `antigravity-intel`. **Toque**: este documento,
> `state/mesh_board.json`, `docs/IDEAS_BACKLOG.md`, `docs/COORDINATION.md`,
> `state/mesh_journal.ndjson` y los tres ficheros nuevos de `deploy/`.
> **NO se toca**: `rust-core/src/cockpit_server.rs`, `rust-core/src/mcp_ext.rs`
> ni ficheros de otros agentes.

---

## 0. Resumen ejecutivo

| | |
|---|---|
| Tareas en el backlog | **2** (no 1: la segunda estaba oculta tras el `last_output` recortado) |
| Tarea viva y sana | **0** — la recurrente (`task_442838`) llevaba **3 días** sin ejecutarse |
| Tarea **nunca** ejecutada | **1** (`task_553653`, `minimax_bootstrap`, `run_at` 2026-11-19 12:09:13) |
| Último ciclo completo | **#37**, terminado 2026-11-19 10:20:07 (211 s, `success`) |
| Ciclo #38 | **arrancado 10:21:33 y muerto sin dejar rastro** (ni log, ni estado, ni error) |
<<<<<<< HEAD
=======
| **Reinicio del backlog** | ✅ **2026-11-22 01:11:10 → 01:14:36**: el runner volvió a ejecutar el **ciclo #38 completo** (`BUILD: PASS`, 206 s) y `megadaemon_state.json` pasó a `cycle_count: 38` (§5.1) |
>>>>>>> agent/antigravity-backlog
| Trabajo conservado | **26 conversaciones (309 MB)**, 97 artefactos en `brain/`, 14.494 funciones recuperadas, 923 descompiladas, 36 builds OK |
| Trabajo perdido | las **27 h 25 min** en las que el daemon no corrió (18-Nov 06:51 → 19-Nov 10:16) y el ciclo #38 |
| Causa del "petó" | el proceso **murió sin registrar nada** (ni crash, ni error) ⇒ **kill**, no excepción |
| Por qué no volvió | **no hay supervisor ni autostart**: el planificador vive dentro del proceso de la app y no se relanza solo |
| Arranque | **aplicado en esta rama**: runner con lock+timeout+log con fecha+estado JSON, y supervisor con catch-up y `service.d` |

---

## 1. El backlog recuperado (íntegro)

`tasks.json` son **2091 B** y contiene un **array JSON de 2 tareas**, no una.
La segunda estaba oculta porque el `last_output` de la primera (1.039 B) se
comía media salida del `cat`. Volcado real (TSV, generado con
`deploy/antigravity-tasks.py` y ejecutado en el móvil):

```
id              name                            enabled trigger  interval last_run_status dur last_run            digest
task_442838     Snatcher-MegaDaemon-SuperLoop   true    interval 300     success           211 2026-11-19T10:20:08  395134-90
task_553653     minimax_bootstrap               true    delay    -       (nunca)           -   -                    1265228-164
```

### Tarea 1 — `task_442838` · "Snatcher-MegaDaemon-SuperLoop" (recurrente)

```json
{
  "id": "task_442838",
  "name": "Snatcher-MegaDaemon-SuperLoop",
  "command": "python3 /data/user/0/com.agy/files/scummvm/tools/re_pipeline/snatcher_megadaemon.py --once",
  "prompt": "", "project": "", "conversation_id": "", "wake_app": false,
  "trigger": { "type": "interval", "interval_sec": 300, "last_run": 1795080093.7521942 },
  "enabled": true,
  "created_at": 1794961442.8382556,
  "last_run_status": "success",
  "last_run_duration": 211,
  "last_run_ts": 1795080008.0533557,
  "last_output": "…1.039 B…"
}
```

* `created_at` 1794961442 = **2026-11-18 01:24:02**.
* `last_run_ts` 1795080008 = **2026-11-19 10:20:08** (fin del ciclo #37, 211 s).
* `trigger.last_run` 1795080093 = **2026-11-19 10:21:33** ⇒ el **disparo del
  ciclo #38**: el daemon lo arrancó y nunca volvió a escribir.
* `last_output` (lo que el daemon guardó del ciclo #37, recortado a ~1 KB):

```
================================================================================
  STARTING RECURSIVE CYCLE #37
================================================================================

================================================================================
  SYSTEM TUNING: ELEVATED HARDWARE OPTIMIZATION & ROOT GOVERNOR
================================================================================

================================================================================
  CYCLE PHASE 1: MULTI-ARCH OMNISCIENT AUDIT
================================================================================

[2026-11-19 10:16:37] [MEGADAEMON] [AUDIT     ] PS1 SLPS_001.54: 561152 bytes | PC=0x800A0800 Load=0x80028000
[2026-11-19 10:16:37] [MEGADAEMON] [AUDIT     ] Saturn MAIN_L.BIN: 259192 bytes | Entry=0x06004000 (SH-2)
[2026-11-19 10:16:37] [MEGADAEMON] [AUDIT     ] PCE Super CD Bank 16 Scene 6: 4096 bytes | Entry=0x2DF9 (HuC6280)
[2026-11-19 10:16:37] [MEGADAEMON] [AUD…      ← (aquí corta el recorte)
```

### Tarea 2 — `task_553653` · "minimax_bootstrap" (una sola vez, **NUNCA CORRIDA**)

```json
{
  "id": "task_553653",
  "name": "minimax_bootstrap",
  "command": "bash /data/user/0/com.agy/files/dumper-web/antigravity-android-repo/modules/runner/bootstrap_core.sh >> /data/user/0/com.agy/files/agy_standalone/bootstrap.log 2>&1",
  "prompt": "", "project": "", "conversation_id": "", "wake_app": false,
  "trigger": { "type": "delay", "run_at": 1795086553.6536741, "delay_sec": 0 },
  "enabled": true,
  "created_at": 1795086553.6536999
}
```

* `created_at` = `run_at` = 1795086553 = **2026-11-19 12:09:13** ⇒ se creó para
  dispararse **en el acto** (disparo `delay`, `delay_sec: 0`).
* **No tiene `last_run_status`, ni `last_run_ts`, ni `last_output`** ⇒ nunca
  corrió.
* **Prueba de que no corrió**: su salida tenía que ir a
  `/data/user/0/com.agy/files/agy_standalone/bootstrap.log` y **ese fichero no
  existe** (`tail` → rc=1, "No such file or directory"). En cambio
  `bootstrap_core.sh` **sí** existe (2.492 B, 12:08) y el directorio
  `agy_standalone/` **sí** tiene artefactos de 12:06–12:17 (`bin/`, `certs/`,
  `core_output.log`, `probe.log`, `probe_core.log`, `run_probe.sh`,
  `standalone.{json,pid,pgid,splitter_pid}`) ⇒ alguien lo lanzaró **a mano**,
  fuera del daemon, pero **la tarea programada quedó colgada para siempre**.

---

## 2. Qué trabajo se conserva y qué se perdió

### 2.1 Se conserva (medido)

| dónde | qué | tamaño / fecha |
|---|---|---|
| `conversations/` | **26 bases de datos** de conversación (una por UUID de `brain/`) | **309 MB** |
| `brain/9dfa200c-b4cf-408f-9f67-d9bee2bd61a4/` | **97 artefactos** del cliente: 52 `.png`, 21 `.json`, 20 `.md`, 1 `.html`, 1 `.gif` | informe de RE, plan de GPU Seere, blueprint Figma, capturas de PS1/Saturn/PCE/SEGA CD, `snatcher_vla_playtest.gif` |
| `brain/<otros 28 UUID>/` | un `scratch/` por conversación | 2026-11-17 → 2026-11-21 |
| `annotations/` | 2 ficheros de anotación | — |
| `mcp/` | 4 cachés de servidor MCP: `megadaemon-minimax`, `gemini-api_gemini-api-docs`, `data-agent-kit-plugin_dak-notebook`, `data-agent-kit-plugin_dak-viz` | el puente al daemon **sigue en disco** |
| `agy_standalone/` | núcleo standalone + sondas (`core_output.log` 11.932 B, `probe.log` 5.179 B, `probe_core.log` 18.695 B, FIFO `core_stream.fifo`) | 2026-11-19 12:06–12:17 |
| `antigravity_state.pbtxt` | onboarding completo, `installation_uuid 5b21c18a-…`, `last_selected_agent_model = MODEL_PLACEHOLDER_M319` | 866 B, 2026-11-21 22:04 |
| `daemon/daemon.log` | **58.594 B / 860 líneas** (356 de traza; el resto son continuaciones de los resúmenes multi-línea) | mtime 2026-11-19 10:21 |

**El estado real del motor** (`scummvm/tools/re_pipeline/megadaemon_state.json`,
6.130 B, mtime 2026-11-19 10:20):

```json
{
  "cycle_count": 37,
  "start_timestamp": 1794961419.673968,      // 2026-11-18 01:23:39
  "last_cycle_timestamp": 1795080005.882835,  // 2026-11-19 10:20:05
  "total_functions_recovered": 14494,
  "total_decompiled_functions": 923,
  "total_hardware_traps": 37,
  "scummvm_build_passes": 36,
  "scummvm_build_failures": 1,
  "platforms": {
    "ps1":    { "functions": 14494, "verified": false },
    "saturn": { "functions": 0,     "verified": false },
    "pce":    { "scenes": 0,        "verified": false },
    "msx2":   { "palettes": 256, "font_glyphs": 596, "verified": false },
    "pc88":   { "ports": 7,         "verified": false },
    "segacd": { "subcode_size": 53110, "verified": false }
  }
}
```

⇒ **El trabajo de reverse NO se perdió**: 14.494 funciones recuperadas, 923
descompiladas, 37 traps, 36 builds correctos, 1 fallido, y el historial
completo de los 37 ciclos con su duración (248 s → 570 s, **creciente**).
Lo que se perdió son **ejecuciones**, no resultados: 27 h de huecos y el
ciclo #38.

### 2.2 Se perdió

1. **El ciclo #38** (arrancado 2026-11-19 10:21:33): no dejó salida, ni
   entrada de log, ni actualización de `megadaemon_state.json` (que sigue en
   `cycle_count: 37` con mtime 10:20) ⇒ se perdió **a mitad**, sin rastro.
2. **La ventana 2026-11-18 06:51:03 → 2026-11-19 10:16:33** (27 h 25 min):
   el daemon no escribió absolutamente nada (ver §3).
3. **La salida completa de los ciclos**: el daemon recorta `last_output` a
   ~1 KB (1.039 B medidos) ⇒ el enjambre **nunca pudo leer** qué hizo el ciclo.
4. **La tarea `minimax_bootstrap`**: nunca ejecutada (y su log, inexistente).

---

## 3. Diagnóstico del corte (con evidencia)

### 3.1 VERIFICADO — el micro-daemon murió en silencio, con un "Worker started" como último rastro

`daemon.log` termina **exactamente** en el arranque del ciclo #38:

```
[2026-11-19 10:20:07] Task [Snatcher-MegaDaemon-SuperLoop] Completed successfully in 211s. Output summary: …
[2026-11-19 10:20:07] Notification: Task Completed successfully: Snatcher-MegaDaemon-SuperLoop - Finished in 211s. …
[2026-11-19 10:21:33] Worker started for task [Snatcher-MegaDaemon-SuperLoop] (ID: task_442838)
[2026-11-19 10:21:33] Worker started for task [Snatcher-MegaDaemon-SuperLoop] (ID: task_442838)
```

* Esa es la **última línea del fichero** (mtime 2026-11-19 10:21). No hay
  después ni un "Completed", ni un "failed".
* `grep -c -i -E "error|fail|fatal|exception|kill|abort|traceback|oom|crash|refus|denied"`
  sobre las 860 líneas ⇒ **0**. El daemon murió **sin registrar un error**.
  (Cada línea sale duplicada en el log: el logger escribe dos veces.)

### 3.2 VERIFICADO — no fue un crash de la app (no hay volcado)

* `crashes/` sólo contiene **2 ficheros de 0 bytes**:
  `crash_4221_…log` (2026-11-20 02:18) y `crash_3492_…log` (2026-11-21 19:23).
  **Ninguno es del 19-Nov.**
* El PID 3492 **sigue vivo ahora mismo** (proceso `libloader.so`, 428 MB RSS)
  ⇒ esos ficheros de 0 B son **marcadores de proceso**, no volcados de crash.
* `/data/system/dropbox/` (126 entradas) **no tiene ninguna del 19-Nov**: la
  más antigua es `SYSTEM_BOOT@1795156980589` = **2026-11-20 07:23**.
* `/data/tombstones/` no aporta nada de esta app.

⇒ **No hay crash dump ⇒ fue un KILL (SIGKILL), no una excepción.**

### 3.3 VERIFICADO — el proceso ya no existe y su PID se recicló

* `daemon/daemon.pid` contiene **`6905`** (4 B, escrito 2026-11-19 10:16).
* El PID **6905** hoy es **`io.github.mekhontsev.magicdesk`** (otra app) ⇒ el
  daemon ya no está; su PID lo reutilizó el sistema.
* `ps -A | grep -i agy` ⇒ **no hay ningún proceso `agy-daemon`**; sólo vive
  `com.agy` (PID 3091, arrancado ~2026-11-21 19:23, RSS 108 MB).

⇒ **El planificador de tareas vive DENTRO del proceso de la app.** Cuando ese
proceso muere, el backlog se queda colgado para siempre.

### 3.4 VERIFICADO — los reinicios tampoco lo salvan (y el reboot fue manual)

* `dropbox` registra **4 reinicios** posteriores:
  `SYSTEM_BOOT@1795156980589` (20-Nov 07:23), `@1795202713076` (20-Nov 20:25),
  `@1795226067474` (21-Nov 02:55), `@1795234711429` (21-Nov 05:18).
* El kernel retenido del arranque anterior cierra con:

  ```
  [108868.272462] init: restart_handler_list -> exynos_restart_handler -> [exynos_reboot] call +
  [108868.272527] init: Reboot command: ( (uid: 0 pid: 25339 processName: app_process))
  ```

  ⇒ **reboot ordenado por `app_process` con uid 0** (shell root), no un
  cuelgue del sistema. Y `108868 s` de uptime ⇒ ese arranque empezó el
  **2026-11-18 23:11**, así que la muerte del daemon (19-Nov 10:21) ocurrió
  **dentro** de ese arranque: el reboot no puede explicar el corte.
* Aun así, desde la muerte del daemon (19-Nov 10:21:33) hasta ahora
  (22-Nov 01:00) han pasado **2 días 14 h 39 min** sin que se ejecute **ni
  una** de las dos tareas, con el móvil **reiniciado 4 veces** y la app
  **viva** desde el 21-Nov 19:23.

### 3.5 VERIFICADO — el corte del log: dos ventanas y un hueco de 27 h

Histograma de marcas de tiempo en `daemon.log` (860 líneas):

```
42 × 2026-11-18 01h   60 × 02h   60 × 03h   66 × 04h   64 × 05h   50 × 06h
14 × 2026-11-19 10h
```

* Ventana 1: **2026-11-18 01:16:52** ("Antigravity background micro-daemon
  loop started.") → **2026-11-18 06:51:03**.
* Ventana 2: **2026-11-19 10:16:33** → **2026-11-19 10:21:33** (muere aquí).
* Hueco: **27 h 25 min 30 s** sin una sola línea.

⇒ El daemon tampoco era estable. Concretamente, estuvo
**apagado 27 h** y en su segunda ventana sólo pudo hacer **un** ciclo
completo antes de morir ⇒ no es un proceso estable, es un proceso **que cae
y nadie lo levanta**.

### 3.6 VERIFICADO (contexto de recursos, **no** causalidad) — memoria y carga

Medido a las 01:00 del 22-Nov:

```
MemTotal: 10.829.480 kB   MemFree: 466.292 kB   MemAvailable: 4.601.168 kB
swap usada: 3.341 MB de 4.095 MB        load average: 22.41 23.04 21.14
```

* El móvil está en **presión de memoria seria** (swap al 82 %, 466 MB
  libres) y el kernel está **señalizando con `OomAdjuster`** a diario.
* Los mayores consumidores: `system_server` 509 MB, `libloader.so` (la propia
  app Antigravity) 428 MB, YouTube 362 MB, `webview` sandbox 295 MB,
  `systemui` 270 MB, `com.minimax.ai` 266 MB.
* El propio ciclo del backlog es un trabajo pesado (RE + builds + Frida): es
  el mismo perfil que ya consumía el móvil 37 veces.

### 3.7 NO VERIFICADO

* **La señal concreta que mató al proceso el 19-Nov 10:21:33.** No hay
  `dmesg` de ese arranque (el buffer de kernel retenido por el dropbox sólo
  cubre los últimos segundos **antes** del reboot del 20-Nov 07:23: 1.097
  líneas con timestamp desde `108868 s`, que es el instante del reboot), no
  hay tombstone, no hay entrada de dropbox del 19-Nov y el propio daemon no
  dejó ni un error.
* **Que fuese OOM.** La presión de memoria está medida (§3.6) y es el
  candidato más plausible, pero **no está probado** para esa marca de tiempo:
  si el `lmkd`/OOM killer hubiera actuado, habría entrada en el log de
  dropbox del 19-Nov y **no la hay**.
* Que el usuario cerrase la app a mano, o que el propio script ("ROOT
  GOVERNOR", `rish`, `renice -n -10`, cpufreq) matara a su propio árbol de
  procesos: **no se puede distinguir** sin el log del kernel de ese arranque.
* Si el corte del 18-Nov 06:51 → 19-Nov 10:16 fue un cierre manual de la app,
  un OOM o un reinicio: **NO VERIFICADO** (no hay `SYSTEM_BOOT` del 18-Nov;
  el dropbox sólo conserva desde el 20-Nov 07:23).

---

## 4. Defectos medidos del diseño (el backlog técnico)

| # | defecto | evidencia | arreglado en esta rama |
|---|---|---|---|
| D1 | **Solapamiento**: `interval_sec = 300` pero el ciclo tarda **211 s** (último) y **hasta 570 s** (historial) ⇒ dos workers a la vez | `tasks.json` + `megadaemon_state.json.history` | ✅ lock atómico (`mkdir`) en el runner |
| D2 | **`python3` no está en el PATH del sistema**: medido `python3: inaccessible or not found` (rc=127); sólo existe en `/data/user/0/com.agy/files/runtime/bin/python3` (Python 3.14.10) ⇒ el comando guardado sólo funciona dentro de la app | `which python3` (rc=127) | ✅ el runner mete `runtime/bin` en el PATH |
| D3 | **Salida recortada a ~1 KB** ⇒ el enjambre no puede leer el resultado | `last_output` = 1.039 B | ✅ log con fecha completo + estado JSON |
| D4 | **El estado sólo se persiste al final del ciclo** ⇒ un ciclo muerto a mitad se pierde entero | `megadaemon_state.json` mtime 10:20 vs. worker 10:21:33 | ⚠️ mitigated (el log del ciclo queda aunque se muera a mitad) |
| D5 | **Sin lock** en el daemon | por diseño | ✅ |
| D6 | **Sin timeout**: un ciclo colgado se queda colgado para siempre | por diseño | ✅ `timeout` con valor por defecto (900 s) |
| D7 | **Sin supervisor ni autostart**: un reinicio o la muerte del proceso apagan el backlog para siempre | §3.3–3.4 | ✅ supervisor + `service.d` |

---

## 5. Arranque propuesto y aplicado en esta rama

Tres ficheros nuevos (nada existente tocado):

| fichero | qué hace |
|---|---|
| `deploy/antigravity-task-runner.sh` | **un** ciclo, siempre: lock atómico, preflight, `timeout` duro, **log con fecha** (`logs/cycle-AAAAMMDD-HHMMSS.log`), **estado legible** (`last_status.json` con `ok/rc/duración/ciclo/motivo/log`), rotación de logs, y digest del comando (**nunca el comando en crudo**, por si algún día lleva credenciales) |
| `deploy/antigravity-tasks.py` | vuelca `tasks.json` en TSV/JSON **sin credenciales** y en modo `--pending` saca las tareas `delay` con `run_at` pasado que **nunca corrieron** (detecta `task_553653`) |
| `deploy/antigravity-task-supervisor.sh` | bucle que (a) hace **catch-up** de las pendientes, (b) lanza el runner cada N s, (c) **no pisa** al daemon de la app si está vivo (`daemon.log` tocado hace poco ⇒ el daemon es el dueño), y (d) `--install` deja un `service.d` de Magisk (`/data/adb/service.d/97-antigravity-tasks.sh`) ⇒ **arranca solo tras cada reinicio**, que es el mecanismo que ya usa este móvil (`91-adb-tcp.sh`, `98-cloudflared.sh`, `99-megadaemon.sh`) |

### 5.1 Verificado en el móvil (por root, 2026-11-22)

| prueba | resultado |
|---|---|
| `antigravity-tasks.py` | vuelca las **2** tareas; `--pending` saca **`task_553653`** (nunca corrida) ✅ |
| `antigravity-task-runner.sh --selftest` | preflight OK (`python3 → Python 3.14.10`), lock OK (el segundo intento se salta), log con fecha escrito, estado JSON escrito ✅ |
| ciclo simulado (`echo 'STARTING RECURSIVE CYCLE #99'`) | `ok:true`, `cycle:"99"`, log con fecha ✅ |
| **timeout** (comando `sleep 30`, `MAX_RUNTIME=3`) | `rc:124`, `reason:"timeout-max-runtime"` ⇒ **no se cuelga** ✅ |
| **concurrencia** (dos runners a la vez) | el segundo: "hay otro ciclo en marcha" y se sale sin tocar nada ✅ |
| `antigravity-task-supervisor.sh --status` | "daemon de la app: sin señal -> el supervisor puede correr el ciclo" ✅ |
<<<<<<< HEAD
=======
| **CICLO REAL #38** (el comando de `tasks.json`, sin tocar) | **✅ `rc:0`, `cycle:"38"`, 206 s**, log íntegro (no los 1 KB del daemon) y `CYCLE #38 COMPLETE (205.11s) \| BUILD: PASS` ⇒ **`megadaemon_state.json` pasó de `cycle_count: 37` a `38`** (`last_cycle_timestamp` 1795306476 = 2026-11-22 01:14:36). **El flujo que se había petado vuelve a correr y deja estado legible.** |
>>>>>>> agent/antigravity-backlog

> **Nota de diseño (medida)**: `flock` en este móvil es el de **toybox
> 0.8.9-android** y su sintaxis es `flock [-sxun] fd` ⇒ **no** admite
> `flock -n 9` sobre un fd abierto por el shell (`Bad file descriptor`) ni
> `flock -n fichero -c cmd` (`Unknown option 'c'`). Por eso el lock es un
> **`mkdir` atómico** con detección de lock huérfano por antigüedad.

### 5.2 Cómo lo lee el enjambre

```bash
# estado del último ciclo (JSON)
cat /data/local/tmp/megadaemon/antigravity/last_status.json

# logs con fecha (el último es el más reciente)
ls -t /data/local/tmp/megadaemon/antigravity/logs/ | head

# el backlog completo, sin credenciales
/data/user/0/com.agy/files/runtime/bin/python3 \
  /data/local/tmp/megadaemon/antigravity/antigravity-tasks.py
```

### 5.3 Cómo se activa (decisión del dueño)

```bash
# 1) instalar (copia + service.d + arranca el bucle)
sh /data/local/tmp/megadaemon/antigravity/antigravity-task-supervisor.sh --install

# 2) sólo el catch-up de lo pendiente, sin bucle:
sh /data/local/tmp/megadaemon/antigravity/antigravity-task-supervisor.sh --status
```

> ⚠️ **NO se ha dejado el bucle corriendo en el móvil a propósito**: el
> dispositivo está en **presión de memoria** (§3.6: 466 MB libres, swap al
> 82 %) y el ciclo es un trabajo pesado. Arrancarlo a la fuerza en este
> momento puede tumbar la sesión de otros agentes. Lo que **sí** está
> instalado y probado es el mecanismo; el `catch-up` de `task_553653` y el
> reinicio del ciclo #38 los lanza `--install` (o `--start`) en un momento de
> RAM reasonable.

---

## 6. Pendientes que dejo señalados (no los hago yo)

1. **`task_553653` (`minimax_bootstrap`)** sigue sin correr: su `run_at`
   (2026-11-19 12:09:13) ya pasó, así que el catch-up la Coggerá en cuanto se
   active el supervisor. **Ojo**: `bootstrap_core.sh` escribe en
   `agy_standalone/` y deja un bearer de Google en claro en `probe.log`
   (ya está anotado en `docs/ANTIGRAVITY_INTEL.md`): rotar ese token antes de
   relanzar.
2. **`bootstrap.log` no existe** ⇒ no hay histórico que recuperar de esa
   tarea; el primer catch-up empieza de cero.
3. Los **`verified: false`** de todas las plataformas (§2.1) ⇒ el motor
   declara 14.494 funciones pero **no ha verificado ninguna**; el backlog
   "verde" del cliente es más una intención que una prueba.
4. **El corte de 27 h del 18-Nov** merece una causa: si fue un OOM, el
   `lmkd` está matando procesos de la app ⇒ conviene bajar la carga del
   móvil antes de reactivar el bucle de 300 s.
5. `daemon.log` **duplica cada línea** (bug del logger del cliente) ⇒ dobla
   el tamaño del log (58 KB) y hace más difícil el grep. No lo he tocado (es
   del cliente).

---

## 7. Trazabilidad

| afirmación | de dónde sale |
|---|---|
| 2 tareas, sus campos, sus disparadores | `daemon/tasks.json` (2.091 B) leído por root |
| ventana del log, líneas duplicadas, 0 errores | `daemon/daemon.log` (58.594 B, 860 líneas) |
| PID reciclado | `daemon/daemon.pid` (6905) vs. `/proc/6905/cmdline` = `io.github.mekhontsev.magicdesk` |
| sin volcado de crash | `crashes/` (2 × 0 B) + `/data/system/dropbox/` (126 entradas, la más antigua 20-Nov 07:23) |
| reinicios | `dropbox/SYSTEM_BOOT@…` ×4 + kernel `Reboot command: (uid: 0 pid: 25339 app_process)` |
| estado del motor | `scummvm/tools/re_pipeline/megadaemon_state.json` (6.130 B) |
| trabajo conservado | `brain/` (29 entradas, 97 artefactos), `conversations/` (26 DB, 309 MB), `agy_standalone/`, `mcp/` (4) |
| `python3` fuera del PATH | `which python3` → rc=127; `runtime/bin/python3 -V` → Python 3.14.10 |
| presión de memoria | `/proc/meminfo`, `free -m`, `/proc/loadavg`, `ps -A` |
| runner/supervisor probados | ejecución en el móvil por root, 2026-11-22 01:08–01:12 |
