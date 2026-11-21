# PLAN MAESTRO v15 — S21 Ultra (Exynos 2100) + Enjambre MegaDaemon

> Sustituye al `MASTER_PLAN.md` (v14) como plan de referencia. El v14 era
> "MegaDaemon y su web"; **este es el plan del móvil y del enjambre**.
> Fecha: 2026-11-20 · Autor: agente General (cockpit-audit-v14) · Tablón: `docs/COORDINATION.md`
>
> **Regla de oro**: si una tarea no está en este documento **ni** en el tablero
> (`docs/COORDINATION.md` + `board.sqlite` de cada instancia), no existe.

---

## 0. Diagnóstico: por qué "no avanzamos" (5 causas, con su contramedida)

| # | Causa real (medida) | Contramedida de este plan |
|---|---|---|
| 1 | **El enjambre habla de MegaDaemon y solo de MegaDaemon.** 19 sesiones, casi todo en refinar la web/cockpit. Nadie tiene asignado "sacar más del móvil" ni "migrarlo". | **W2–W5** dan trabajo de dispositivo a agentes que hoy no lo tienen, con entregables verificables. |
| 2 | **MegaDaemon no es un agente: es un servidor de tools.** No decide, no habla, no reclama tareas por su cuenta. El LLM y el enjambre solo se usan cuando un humano llama a una tool. | **W0 — VOZ**: el daemon es un nodo más del enjambre con identidad, voz y bucle autónomo (reclamar → decidir con LLM → actuar → difundir). |
| 3 | **La UI es mala y además hay tres UI** (plantilla v1, `cockpit_v2.html`, `webui.rs` con 24 rutas v2) sin una fuente de verdad. | **W1** — una sola UI, mobile-first, con el diseño de `web/webui_v2.css` y las 10 vistas, y criterio de "hecho" medible. |
| 4 | **El móvil no está inventariado ni respaldado.** 225 GB al 98%, 184 MB libres en el incidente de OOM, y sin inventario verificable. Un brick lo destruye. | **W2** (inventario) y **W3** (backup ultra completo con verificación de restore) **antes** de W4. |
| 5 | **Rotamos credenciales en el chat**, y los agentes siguen con la vieja → 403/530 que *parecen* "el móvil está caído". | **W7** — la fuente de verdad es el **fichero** (`md-secrets/instance.env`) y el canje `POST /api/session/exchange` (key en el cuerpo → token vigente). Prohibido leer credenciales del chat. |

---

## 1. Los 8 workstreams (W0–W7)

Cada uno: **dueño**, **ficheros** (un fichero = un agente), **estado real medido**, **Definition of Done**.

### W0 · VOZ — que MegaDaemon sea un agente (no un servidor de tools)

**Dueño**: agente General (este) para el módulo; **cableado**: principal (`lib.rs`/`main.rs`/`mcp_server.rs`).
**Ficheros nuevos**: `rust-core/src/agent_voice.rs` (mío) + `rust-core/tests/agent_voice_smoke.rs` (mío).
**Estado real**: ya existen `src/mesh.rs` (tablón + journal + peers), `src/agent_core.rs`, `src/capability.rs`,
`src/providers.rs` (13 proveedores), `src/coordination.rs` (board SQLite) y la tool `swarm_chat`.
**Lo que falta (y es el fallo de diseño que señalas)**: nadie conecta esas piezas:
el daemon no se registra como agente, no reclama tareas, no decide con un LLM y no habla con los demás.

**DoD**:
1. `agent_voice::VoiceProfile` = identidad del daemon como agente (`node_id`, `role`, `capabilities`, `instance`).
2. `agent_voice::plan_next(ctx)` — dado el tablero + el estado real, devuelve la siguiente acción
   (`Claim{task_id}` | `Ask{peer,question}` | `Speak{text}` | `Do{tool,args}` | `Idle{reason}`).
   **Puro y testeable**: el LLM entra por un trait `Brain`, con un `Brain` de pruebas determinista.
3. `agent_voice::act(ctx, action)` — ejecuta la acción por la **tool registry** (la misma que usa `/api/tool/<n>`),
   y **difunde** el resultado por los 4 canales (journal, sync, board, `swarm_chat`).
4. `agent_voice::speak_to_peer(ctx, peer, question)` — pregunta a otro nodo por el bus y trae la respuesta.
5. Tool MCP nueva: `megadaemon_voice` (accion: `plan` | `act` | `speak` | `whoami`) → la usan los agentes
   para **preguntarle al entorno real** (el daemon es el que puede ejecutar en el móvil).
6. Un bucle autónomo **opt-in** (`--voice` / `MEGADAEMON_VOICE=1`): cada N s, `plan_next` → `act` → `difundir`.
   Con límites dura: máx. 1 acción por tick, budgets, cooldown y **nunca** reinicios sin avisar.

### W1 · Interfaz web (la "mierdísima" interfaz)

**Dueño**: agente de la UI v2 (`e81fade1`) — `web/cockpit_v2.html`, `web/webui_v2.css`, `rust-core/src/webui.rs`.
**Apoyo**: este agente (botón de prompt, `web/md_prompt_gen.js`, ya inlined en el móvil).
**Estado real**: `webui.rs` tiene **24 rutas v2 y 23 tests verdes**; `cockpit_v2.html` tiene **10 vistas**;
el diseño visual vive en `web/webui_v2.css`. **Nada de eso está cableado en el binario**: falta
`pub mod webui;` + `webui::dispatch(...)` + servir `/v2`. El móvil sirve hoy la v1 (89 KB).
**DoD**:
1. `/v2` sirve la UI nueva (mobile-first, sin CDN, modo claro/oscuro, AA, foco visible, `prefers-reduced-motion`).
2. Las 10 vistas funcionan con **1 fetch por vista** (agregadores), no ~20.
3. Tabs: **Resumen · Nodos · Tablero · Agente · Delegación · Alertas · RE/Apps · Archivos compartidos · Reparto · Sistema**.
4. La pestaña **ENJAMBRE** muestra los hallazgos del nodo (lee `<state>/mesh_journal.ndjson` y `sync/re_*.ndjson`).
5. El botón **📋 Prompt agente externo** (ya implementado, `web/md_prompt_gen.js`) generates el prompt vivo y
   permite copiar/descargar; **integrado en la v2** (no en un parche en caliente).
6. Cero "fantasmadas": si una ruta no está desplegada, la vista dice el motivo y el path que falta.

### W2 · Inventario completo del S21 Ultra (lo que nadie ha hecho)

**Dueño**: agente de inventario (`6d7ffc8f`).
**Ficheros**: `rust-core/src/device_inventory.rs` + `rust-core/tests/device_inventory_smoke.rs`.
**Estado real**: `system_overview` da SoC/RAM/apps/selinux/magisk, pero **no** hay inventario persistido
ni verificable; el disco está al 98% y el móvil ya tuvo un OOM.
**DoD**: un `inventory.json` (versionado, con sha256 de cada fichero de referencia) que contenga:
- Hardware: SoC (Exynos 2100 / s5e9840), CPU/GPU/NPU (Mali-G78 MP14, EDEN), RAM+swap, pantalla, sensores, cámaras, módem.
- Software: build de ROM (nivel de parche, CSC), kernel, SELinux, Magisk, Root,_superuser, módulos, Work Profile.
- Particiones y boot: tabla de particiones, `getprop` de boot, AVB/vbmeta flags, TEE/GREEN TEA/TEEGRIS.
- Apps: inventario completo (paquete, versión, hash de firma, si es sistema, si está habilitada, permissions de riesgo).
- Red: interfaces, SSID/BSSID guardados, DNS, proxies, VPN; ADB keys autorizadas.
- Cuentas Google/Samsung (solo presencia y estado, **nunca** tokens).
- Espacio: mapa de uso por directorio (para saber qué se puede tocar sin ladrillar).
- **Todo en Drive y en el repo** (manifest + sha256), no en el chat.

### W3 · Backup "ultra completo" (con restore probado)

**Dueño**: agente de backups (propone: `6cb64e00` extractor + `6d7ffc8f`).
**Ficheros**: `rust-core/src/backup_plan.rs`, `deploy/backup_phone.sh`, `rust-core/tests/backup_plan_smoke.rs`.
**Estado real**: hay un backup de 35,72 GB en Drive del nodo RE (material estático del ROM), **pero no**
un backup del *estado del usuario* del móvil ni un restore probado.
**DoD** — 4 capas, cada una con sha256 y verificación:
1. **Nivel 0 — volátil**: `/data/local/tmp/megadaemon/state` (tablero, tokens, audit), `board.sqlite`, journal, sync.
2. **Nivel 1 — ROM**: boot/init/etc (el pack del nodo), particiones de solo lectura.
3. **Nivel 2 — datos de usuario**: WhatsApp/Chrome/Samsung apps (con soporte de cifrado: **no** prometer lo que AndroidKeyStore impide), DCIM, Downloads, Documents, /sdcard.
4. **Nivel 3 — credenciales**: **solo** con cifrado propio y clave derivada de hardware, exportable; nada en claro en Drive ni en el repo.
- **Restore probado**: instalar el pack en un emulador o en un dispositivo de repuesto y verificar 3 lecturas reales.
- **Restricción dura del móvil**: 5,9 GB libres ⇒ **todo el backup se hace por streaming a Drive (chunks + sha256)**, nunca copia local intermedia.

### W4 · Migración de ROM (plan automatizado, con anti-brick)

**Dueño**: agente de ROM (propone: `27a19f3a` analizador + `6cb64e00` extractor); **coordinación**: principal.
**Ficheros**: `rust-core/src/rom_migration.rs`, `deploy/rom_migrate.sh`, `rust-core/tests/rom_migration_smoke.rs`.
**Estado real**: hay una imagen boot.img reempaquetada de VN-ROM con `selinux=permissive` (detectado por el nodo) —
**no** es stock y no se ha flasheado nada. El móvil tiene **FMM Lock** (unlock gated por manid/warranty) y AVB encadenado.
**DoD** — el plan es **documentado + dry-run**, el flashing **manual y supervisado**:
1. **Pre-flight automático**: OEM unlock status, FRP/OMI, battery ≥80 % + temperatura, espacio, versión de binario Odin,
   checksum de la ROM, coherencia CSC/OMC (PIT + CSC), versión de bootloader mínima, y **abortar si hay boot.img no stock**.
2. **Selección de ROM**: misma CSC (AMO/EUR), parche ≥ actual, parche de seguridad, región del módem; el
   "latest" que ofrece Samsung NO vale si cambia de CSC → verificación con el CSC real del móvil.
3. **Guarded flash**: 4 pasos (WIPE DATA → BOOTLOADER → AP/CP/CSC → HOME_CSC), con verificación entre pasos
   (hash del AP y del CSC escritos en el móvil antes y después).
4. **Anti-brick**: EFS y particiones de calibración copiadas ANTES y con sha256; Odin en modo Download;
   nunca "Erase Download" sin EFS; plan de recovery documentado (TWRP, sideload de la partición `recovery`,
   o `heimdall`/`fastboot flash recovery`).
5. **Post-flash automático**: primer arranque, re-espera de root, re-instalación de módulos Magisk, y
   **re-ejecución del inventario W2** para comparar antes/después.

### W5 · Ingeniería inversa masiva a Antigravity (y que sea cliente de MegaDaemon)

**Dueño**: agente de RE/`275a2232` (intel de apps y ocultación) + `659db26c` (DEX).
**Ficheros**: `rust-core/src/antigravity_intel.rs`, `rust-core/tests/antigravity_intel_smoke.rs`,
`docs/ANTIGRAVITY_INTEL.md`. Ya existe `docs/antigravity-ecosystem-catalog.md`.
**Estado real**: hay un catálogo del ecosistema Antigravity pero **no** hay reversing del binario real
ni del protocolo de su MCP.
**DoD**:
1. Inventario del cliente Antigravity: binario, plugins, servidores MCP que configura, hooks, permisos, storages.
2. **Qué protocolo habla**: si habla MCP, entonces puede ser **cliente de MegaDaemon** al revés (MegaDaemon como
   servidor MCP ya lo es: `/mcp` con 45 tools) ⇒ el objetivo real es que **Antigravity use el `/mcp` del móvil**.
3. Extracción de sus credenciales de proveedor (solo estructura) y **re-uso de los proveedores ya existentes**
   de `providers.rs` (13 proveedores) para no duplicar.
4. Puente documentado: `Antigravity → http://<móvil>:9090/mcp` (LAN) o por el túnel, con la API key.

### W6 · Conectores: MegaDaemon ↔ MiniMax ↔ cualquier LLM/agente ↔ repos de GitHub

**Dueño**: agente de "Delegación parasitar" (`8b154d6a`) + `6d7ffc8f`; soporte de servidor: `23c1e5eb`.
**Ficheros**: `rust-core/src/service_bus.rs`, `rust-core/src/github_connector.rs` (+ tests), `docs/LLM_SWITCH_PLAN.md` (ya existe).
**Estado real**: `providers.rs` tiene 13 proveedores y `swarm_chat` enruta a un modelo; **pero el daemon no
los usa de forma autónoma** (de eso va W0) y no hay catálogo de servicios *de agentes*.
**DoD**:
1. `service_bus`: catálogo unificado de **modelos** (MiniMax, OpenRouter, Groq, Gemini, GLM, Pollinations, SEERE local)
   y de **agentes/servicios** (otros nodos MegaDaemon, Antigravity como cliente, servidores MCP de terceros).
2. Cada entrada: `id, kind, endpoint, auth_kind, health, latency, cost_estimate, capabilities, enabled`.
3. Selección automática por **tarea** (inferencia / build / análisis / redacción) con **fallback** y **presupuesto**.
4. **Conector de GitHub**: listar/descargar los repos del usuario, leer issues/PRs, y **publicar el estado del
   enjambre como issue o PR** (con `git_ops`/`repo_updates` que ya existen).
5. Todo con el mismo "sin secretos en el estado compartido" y con el **canje de token** como fuente de verdad.

### W7 · Coordinación real ("hablad todos") + preguntas al entorno

**Dueño**: bus de coordinación (`9cb64913`) + principal.
**Ficheros**: `rust-core/src/coordination.rs`, `docs/COORDINATION.md`, `state/*` (tablón compartido).
**Estado real**: el tablón funciona (SQLite + NDJSON + journal), pero **cada agente trabaja en su carril y nadie
pregunta a los demás**; y las preguntas al "entorno real" se hacen a mano, no con una tool.
**DoD**:
1. **Voz de red**: cada agente publica su estado con **una línea** en el tablón y **responde** a las preguntas
   de los demás en ≤1 ciclo. El daemon (W0) también es un oyente.
2. **Tool de introspección del entorno**: una tool que responde preguntas sobre el móvil real
   (`¿qué hay en /dev?`, `¿qué app tiene X?`, `¿cuánta RAM libre?`, `¿qué toca este binario?`) ⇒ los agentes
   **preguntan al entorno, no adivinan**.
3. **Reunión de estado cada 30 min** (no al final de cada tarea): 3 líneas por agente: qué hice / qué toca / qué bloquea.
4. **Anti-monotema**: cada agente tiene prohibido cerrar su tarea sin haber leído el tablón completo; el principal
   reparte **una tarea fuera de su carril** a cada agente cada ola (rotación).

---

## 1.b · MEDIDAS DEL MÓVIL VIVO (2026-11-20, nodo `cloud-sandbox` con `root_exec`)

Esto **cambia el plan**: son 24 artefactos con sha256 real del dispositivo (no del
volcado de ROM), y corrigen supuestos que teníamos por buenos.

| Corrección | Medida | Impacto |
|---|---|---|
| La TEE de Samsung es **`fkeymaster-service`** | **no existe** `/vendor/bin/hw_keymaster` | W5: buscar la firma de claves en `fkeymaster-service`, no en `hw_keymaster` |
| `keystore2` y `gatekeeperd` están en **`/system/bin`** | no en `/vendor/bin` | W5: las rutas del volcado de ROM **no** corresponden a este firmware |
| La cadena AVB son **2 niveles**, no 3 | **no existen** `init_boot`, `vbmeta_vendor` ni `sec` | W4: el pre-flight debe validar 2 niveles, no 3 |
| Particiones reales del mundo seguro | `sec_efs`, `keystorage`, `keyrefuge`, `uh`, `uhcfg`, `spu`, `prism` | W3: el backup de EFS son esas 7, y hay que hashear cada una |
| **AVB `green` con `flash.locked=1` ⇒ bootloader CERRADO con Magisk** | medido en el móvil | **W4: el flash es supervisado sí o sí**; no hay escenario "bootloader abierto". El plan de ROM empieza por *saber si se puede desbloquear* (FMM Lock + manid) |
| `/dev/vertex10` = `0644`, **uid/gid 1000** | `crw-r--r-- 1 system system 82,10` | **W5: el nodo del NPU lo lee cualquier app** ⇒ finding de seguridad de primer nivel (y por eso el P4 del nodo usa la `.so` del **dispositivo**, sha `7c0041ab…`, no la del ROM) |
| `NPU.bin` y `eden_kernel_64.bin` **no están** en el firmware vivo | ausentes en `/vendor/bin` y `/vendor/etc` | **W5: la tabla de ioctls del ROM no vale**; hay que extraerla de `libeden_ud_npu.so` del dispositivo |
| No existen `/dev/vertex10_test` ni `/sys/class/misc/vertex10` | medido | P4: el plan de ioctls no puede apoyarse en esas rutas |

**Lección de método (la que más nos ha costado)**: *la tabla de ioctls del ROM
reconstruido no es la del móvil*. Dos binarios con el mismo nombre y distinto
sha256 ⇒ hay que RE siempre contra lo que se ejecuta en el dispositivo.

### Estado real de las claves de API (medido, y corrige el diagnóstico)

- El tool `api_keys` (mint/revoke/list) **existe en el código** (`rust-core/src/mcp_ext.rs`)
  y `mcp_server.rs` **ya lo enruta** (`mcp_ext::dispatch`), pero **el binario desplegado
  (v13.4.0) es anterior**: `POST /api/tool/api_keys` → **404 `unknown_tool`** y por MCP
  → `Unknown tool: api_keys` (además no está en su `tools/list`).
- El **registro no se sobrescribe**: `generate_extra_key` **añade** (lee + anexa + 0600) y
  `ApiKeyConfig::load()` se ejecuta **en cada petición** ⇒ **no hace falta reiniciar** el daemon.
- El 403 que vio un nodo fue porque su key **nunca estuvo registrada** en
  `state/megadaemon_api.keys` (el registro real tiene 2 claves: la del agente externo y otra).
- **Regla para acuñar una clave (la que falta en el tool)**: (1) escribir en
  `state/megadaemon_api.keys` (0600), (2) **no** hace falta recargar (se relee por petición),
  (3) **verificar** con un `tools/list` real usando la key nueva, y (4) informar **sólo del sha16**.
- **Bug de la misma familia que UI-08 (silencioso)**: el dispatcher genérico responde
  `200 {"status":"executed"}` a **cualquier** ruta desconocida (`POST /api/tools`,
  `POST /api/tool`, `POST /api/api_keys` → los tres 200 "executed"). Debería ser
  `404 unknown_tool`: hoy un cliente cree que ha hecho algo que no ha hecho.

### Consecuencias de P2-bis (medidas por el nodo, ya incorporadas)

- **El material de claves por dispositivo está en `/mnt/vendor/efs/DAK/`** (GAK/SAK EC+RSA
  `private` + `.val` + certificados) y el almacén seguro de la TEE en
  `/tee/srt/<uuid>/<id>/` (`obj_info` + `data_chunk`).
  ⇒ **W3**: un backup de EFS **sin `keystore2`** es incompleto (el motor de claves está en
  `/system/bin`, no en el EFS) ⇒ el backup incluye el binario `keystore2` y su base de datos.
- **`/root/FLL.bin` es la protección anti-rollback** ⇒ **W4** tiene que respaldarlo y
  **verificar su hash antes de tocar la ROM** (si baja de versión el bootloader, FLL lo bloquea).
- Las **7 particiones del mundo seguro** con sha256 calculado **on-device** (el contenido no
  sale del móvil): `keystorage`(sda6,512 KiB) · `keyrefuge`(sda21,52 MiB) · `sec_efs`(sda2,20 MiB) ·
  `uh`(sda9,2 MiB) · `uhcfg`(sda10,4 MiB) · `spu`(sda33,50 MiB) · `prism`(sda29,256 MiB).
- **Tool de introspección del entorno** (`src/env_introspect.rs`, 5 tests verdes): preguntas al
  móvil real (`partitions`, `dev`, `binaries`, `app`, `ram`, `path`, `prop`, `processes`) con
  contrato obligatorio `source` + `ts` + `device`, y `available:false` + `reason` cuando algo no
  existe. **Cableada en `mcp_ext`** ⇒ es lo que evita volver a suponer (`NPU.bin`, `hw_keymaster`).

---

## 2. Orden de las próximas 72 h (lo que desbloquea qué)

```
H+0  ── W0 Voz: módulo agent_voice.rs + tests (este agente)  ──────────┐
H+0  ── W1 UI: /v2 cableada + ENJAMBRE + prompt (agente UI)           │
H+0  ── W2 Inventario: inventario.json + sha256 (agente inventario)    │
H+1  ── W3 Backup L0+L1 a Drive con verificación (agente backups)      │
H+2  ── W3 Restore probado en emulador (agente backups)               │
H+2  ── W4 ROM: pre-flight + selector + guarded flash en DRY-RUN      │
H+4  ── W4 Flash SUPERVISADO (solo con EFS copiado + usuario presente) │
H+6  ── W5 Antigravity: inventario + ¿habla MCP? → cliente del móvil  │
H+8  ── W6 service_bus + conector GitHub                             │
H+12 ── W0 autónomo en el móvil: daemon reclama, decide y difunde     │
H+24 ── Ola 2: W2/W3 re-tras el móvil ya migrado (comparar)           │
```

**Reglas de seguridad (no negociables, y por qué):**
- Nada de flashear sin **EFS copiado y verificado** + usuario presente + batería ≥80 %.
- El móvil **no** compila: 3 GB de RAM y disco al 98% ⇒ builds solo en disco local del sandbox.
- **Ningún secreto en el chat ni en el estado compartido**: fuente de verdad = `md-secrets/instance.env`
  y el canje `POST /api/session/exchange` (key en el cuerpo → token vigente).
- Si la URL del túnel da 530, **no reinicies el túnel**: 530 = conector caído; 502/503/504 = origen caído.
  La URL vigente está en `/data/local/tmp/megadaemon/tunnel.url` en el móvil.

---

## 3. Lo que YA está hecho (no repetir)

| Entrega | Estado |
|---|---|
| 3ª auditoría UI/HTTP/MCP (11 hallazgos vivos, 4 controles) | `rust-core/tests/audit_scout_ui.rs`, informe `docs/AUDIT_V14_UI_HTTP.md` |
| `cockpit_doctor` (diagnóstico puro, estado `not_operational`) | `rust-core/src/cockpit_doctor.rs` + 9 tests verdes |
| `install-phone.sh` (sha256 en el móvil, espera activa, **rollback si la UI queda muerta**) | `deploy/install-phone.sh` |
| Generador de prompt para agentes externos (botón en la web del móvil) | `web/md_prompt_gen.js` + parche en caliente en el móvil |
| RCE root del puente `:3000` (CORS `*` + token inyectado) | corregido en repo **y** móvil, verificado |
| `shm::encode_bmp_24` saturado (el panic que mataba el daemon) | aplicado por el principal (commit `4d1785b`) |
| Canje `/api/session/exchange` arreglado (key en el cuerpo → token) | aplicado por el principal (`d93fa217`) |
| RE del Exynos 2100: P1 NPU/EDEN (11), P2 TEE (9), P3 boot chain (4) | nodo `cloud-sandbox`, en Drive + repo + estado del móvil |
| `webui.rs` con 24 rutas v2 y 23 tests + `cockpit_v2.html` (10 vistas) + `webui_v2.css` | en el árbol, **pendiente de cablear** |
| Recuperación de canales tras el OOM + watchdog de túneles | `deploy/recover_channels.sh`, instalado en el móvil |

---

## 4. Cómo se comprueba (Definition of Done global)

1. **Nada de "fantasmadas"**: cada afirmación con fichero:línea o medición.
2. Cada workstream deja **un test que falla si el trabajo se deshace** (como `audit_scout_ui.rs`).
3. Cada entrega grande se **emite por los 4 canales** del enjambre (journal, sync, board, `swarm_chat`).
4. El **tablón manda**: `docs/COORDINATION.md` + `board.sqlite`. Una línea por agente.
5. Rotación de secretos: **persistir antes de reiniciar**, y el canje como fuente de verdad.

*Documento del agente General (cockpit-audit-v14) · rama `agent/cockpit-audit-v14`.*
