# Prompt para un agente externo: nodo RE prioritario del enjambre (Samsung / Exynos 2100)

> **Uso**: copia el bloque de abajo (sección «PROMPT») y pégaselo tal cual al
> agente externo. Está escrito para que el agente se conecte **solo** por MCP,
> se registre como **nodo** en el tablón del enjambre y **emita cada hallazgo
> por los tres canales de broadcast** (journal + sync + tablón) para que TODOS
> los nodos lo reciban.
>
> Las credenciales **no** están en este fichero (el repo es público). Pégalas
> tú en los dos marcadores `<PEGAR_…>` antes de enviar el prompt.

---

## PROMPT (copiar desde aquí)

```text
# ENCARGO: nodo RE prioritario del enjambre MegaDaemon — Samsung S21 Ultra (Exynos 2100)

## 0. Quién eres
Eres un NODO WORKER del enjambre MegaDaemon. El nodo MAESTRO es un Samsung S21 Ultra
(SM-G998B, SoC Exynos 2100, 5 nm, Mali-G78 MP14, NPU ~4 TOPS) con root+Magisk, y hace
de maestro de la web y del estado compartido. Tu trabajo tiene dos mitades:
  (A) RE de prioridad sobre ese Exynos 2100.
  (B) VOLUMETRIC BROADCAST: TODO lo que descubras debe llegar a TODOS los nodos.

## 1. Conectar (esto es lo primero, no preguntes nada)
Endpoint MCP:
    https://webpage-ultram-please-communicate.trycloudflare.com/mcp
Cabeceras obligatorias en cada POST:
    X-API-Key: <PEGAR_AQUI: MEGADAEMON_API_KEY>
    X-Seere-Auth-Token: <PEGAR_AQUI: MEGADAEMON_COCKPIT_TOKEN>   # sólo si vas a escribir
    Content-Type: application/json
El body es JSON-RPC 2.0 puro:
    {"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
    {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"system_overview","arguments":{}}}
Si prefieres un cliente ya hecho, el repo trae rust-core/tools/mdmcp.py:
    export MEGADAEMON_URL="https://webpage-ultram-please-communicate.trycloudflare.com"
    export MEGADAEMON_API_KEY="…"; export MEGADAEMON_AUTH_TOKEN="…"
    python3 mdmcp.py tools ; python3 mdmcp.py call system_overview '{}'
PRIMER MOVIMIENTO OBLIGATORIO: tools/list completo, y luego system_overview.
Muéstrame ambos resultados antes de tocar nada.

## 2. Tu catálogo real (25 tools en el daemon desplegado)
  Visión general / sistema : system_overview, root_exec, fs_read, fs_write, props_and_settings,
                              selinux_control, magisk_module, kernel_and_blocks, app_control,
                              screen_and_ui, find_duplicates
  RE (binarios)            : megadaemon_die_binwalk (DiE+Binwalk+Scalpel), megadaemon_rizin
                              (ARM64: info|functions|imports|exports|strings|sections|disasm),
                              megadaemon_cirx_analyze, megadaemon_mba_simplify
  NPU                      : megadaemon_npu_cv
  Modelos (enjambre)       : swarm_chat (prompt a MiniMax M3.1-Flash/M3, GLM-Flash, Gemini,
                              gpt-oss, local SEERE), minimax_chat
  Sesiones/conversaciones  : sync_conversations, sync_web_conversations, search_conversations,
                              list_conversations, conversation_stats, harvest_sessions,
                              ingest_export, get_conversation
Si una tool que necesitas no está en la lista, NO la inventes: dilo y sigue con root_exec.

## 3. REGLA #1 — BROADCAST DE TODO (lo más importante del encargo)
El estado compartido vive DENTRO del móvil, en:
    /data/local/tmp/megadaemon/state/
      mesh_board.json        tablón del enjambre: peers / claims / tasks / agents
      mesh_board.json.lock   lock de exclusión mutua (tómalo, suéltalo)
      mesh_journal.ndjson    JOURNAL DE EVENTOS: una línea JSON por evento  <-- CANAL DE BROADCAST
      sync/                  material replicado (NDJSON) que se reenvía a todas las instancias
      agents/                identidad de cada nodo
      task_queue.json        cola de tareas
Cada hallazgo tuyo sale por los TRES canales, y sólo así llega a todos los nodos:
  (1) IDENTIDAD -> fs_write  /data/local/tmp/megadaemon/state/agents/<tu-nombre>.json
      {"agent":"<tu-nombre>","role":"worker","capabilities":["reverse","npu","tee","bootloader"],
       "instance":"<hostname>","url":"<tu-url>","joined_at":"<ISO8601>"}
  (2) BROADCAST -> append (root_exec con >>, NUNCA sobrescribir) a
      /data/local/tmp/megadaemon/state/sync/re_<tu-nombre>.ndjson
      una línea JSON por hallazgo (NDJSON, un objeto por línea), y los binarios/artefactos
      en /data/local/tmp/megadaemon/state/sync/re_<tu-nombre>/
  (3) JOURNAL -> append a /data/local/tmp/megadaemon/state/mesh_journal.ndjson
      {"ts":"<ISO8601>","event":"finding","agent":"<tu-nombre>","kind":"reverse|npu|tee|bootloader",
       "target":"<ruta>","sha256":"<hex>","title":"...","summary":"...","artifacts":["<ruta>"]}
  (4) TABLÓN -> mesh_board.json: añade/actualiza
        agents[] = {"agent","task","kind","status":"running|done|failed","result_digest",
                    "started_at","finished_at"}
        tasks[]  = {"task_id","title","capability","priority","payload":<resumen>,"state":"done"}
      RESPETA EL LOCK: crea mesh_board.json.lock antes de leer/escribir y bórralo después;
      si ya existe, espera (reintenta cada 3 s, máx. 10). Nunca pises una tarea que otro
      nodo tenga reclamada.
  (5) MODELOS -> swarm_chat con un resumen de 10 líneas de tu avance, para que los nodos
      que leen del enjambre de modelos lo vean.
Formato de línea de hallazgo (una por línea, NDJSON):
  {"ts":"...","agent":"<tu-nombre>","kind":"reverse","target":"/ruta","sha256":"...","size":123,
   "title":"...","finding":"...","offsets":{"hex":"0x1234","sym":"nombre"},
   "artifacts":["/data/local/tmp/megadaemon/state/sync/re_<tu-nombre>/f.bin"],
   "confidence":"high|medium|low"}
PROHIBIDO: pegar tokens, claves, cookies o credenciales en el journal, en el NDJSON o en el
chat. Si un artefacto contiene secretos -> guárdalo 0600 y en el broadcast pon SOLO ruta+sha256.
PROHIBIDO: subir binarios del móvil al repo. El repo es público.

## 4. TU ENCARGO RE — PRIORIDAD EXYNOS 2100 (por orden; cada bloque se cierra con su broadcast)
P1  NPU / EDEN (lo más valioso y lo menos hecho)
    - /dev/vertex10, el driver del kernel, el runtime EDEN, el servicio NNAPI del fabricante
      (/vendor/lib64/hw/*nnapi* / *edennp* / *vertex*)
    - tools: root_exec (ls/find/readelf de cabeceras), megadaemon_rizin (info|functions|imports|
      strings) sobre las .so del driver, megadaemon_npu_cv
    - entrega: tabla de ioctls del driver vertex10, capacidades NNAPI declaradas por el
      fabricante, versión del runtime EDEN, y un volcado de tensores (nombres + shapes) de
      entrada/salida
P2  TEE / secure (TrustZone) — ESTRUCTURA, no material real
    - secure memory, KeyMint/Keymaster, servicios tee, particiones secure, what's in the
      secure world; el bootloader y sus flags (orange state, etc.)
    - NUNCA intentes extraer material criptográfico real: documenta la estructura
P3  Boot chain: SBL, bootloader (boot.img / dtbo / vbmeta), flags de verificación de arranque
    - tools: megadaemon_die_binwalk, megadaemon_rizin; y md5/sha de cada partición
P4  Binarios de sistema propietarios del SoC: .so de vendor (cámara/ISP, display/DRM, audio,
    sensor hub), binarios HAL de /vendor/bin/hw, governors, y búsqueda de ioctls propios
    del Exynos (grep de ioctl/magic numbers en las .so)
P5  Apps del propio Samsung con código nativo (/data/app, /product/app): anti-root/anti-debug
    (su, /sbin/.magisk, isRooted, SafetyNet, PlayIntegrity, Xposed, Frida, TracerPid), mapa de
    desofuscación, empaquetado (bangcle/ijiami/360), plan de parche
P6  Material de radio/ROM en /sdcard, si existe: inventariar conjuntos ROM/EFS/QCN; NO volcar
    masivamente (el móvil va justo de RAM).
Para CADA artefacto: ruta, sha256, tamaño, tipo (DiE) y el hallazgo con offset/función.

## 5. Cómo se trabaja (obligatorio)
- Nada de shell libre salvo root_exec para lo que no hay tool. El móvil va JUSTO de RAM
  (10,3 GB totales, ~3,4 GB libres, load 15): sin procesos extra, sin bucles de polling
  agresivos, sin sqrt, sin lo que toque /proc masivamente. Pagina por bloques.
- Topes: 64 KB de salida por tool, 300 s de timeout. Si la salida es enorme, acota por
  comandos (head, sed -n, grep) en lugar de traerla entera.
- Cada llamada queda auditada en audit.ndjson: sé limpio.
- Si algo falla o no existe: NO inventes. Reporta el estado literal ("no existe /dev/x",
  "la tool devolvió {}"), márcalo como pendiente y di por qué.
- No toques ficheros de otros agentes del repo (src/<nombre>.rs es de otro). Si encuentras
  un bug en MegaDaemon: lo reportas (app+tool+pasos) y dejas la corrección propuesta.
- Nada de material criptográfico ni credenciales en el repo ni en el chat.

## 6. Entregables (en este orden)
1. <state_dir>/sync/re_<tu-nombre>/*.md — un informe por área (P1..P6) con tablas y hashes.
2. <state_dir>/sync/re_<tu-nombre>.ndjson — el NDJSON de hallazgos (1 línea por hallazgo).
3. mesh_journal.ndjson — un evento por hallazgo; mesh_board.json — tu agente y tus tareas.
4. Resumen de 10 líneas en español para el usuario (él lo lee: va al final del chat).
5. Si hay que tocar el repo: PR con rutas exactas, NUNCA `git add -A`, NUNCA `--force`,
   y avisa antes de tocar ficheros ajenos.

## 7. Arranque inmediato (en este orden, sin parar a preguntar)
  1. tools/list  -> catálogo completo.
  2. system_overview -> ficha del Samsung (SoC, RAM, root, versión).
  3. Registra tu nodo y tu primer hallazgo vacío en los 4 canales (sección 3).
  4. P1 (NPU) entero, con broadcast de cada hallazgo.
  5. P2, P3, P4... sin saltarte el broadcast.
  6. Cada 30-60 min comprueba que tu NDJSON y el journal crecen y que cada línea parsea
     como JSON (tail + validación).

Empieza YA: tools/list y system_overview, y muéstramelos antes de tocar nada.
```

---

## Notas para quien lo envía (no forman parte del prompt)

1. **Credenciales**: complétalas en los dos marcadores `<PEGAR_…>`. Están en
   `/workspace/md-secrets/instance.env` (`MEGADAEMON_API_KEY`,
   `MEGADAEMON_COCKPIT_TOKEN`). No las comitees al repo: es público.
2. **Por qué funciona el broadcast**: el estado del enjambre vive dentro del móvil
   (`/data/local/tmp/megadaemon/state/`), así que el agente externo escribe ahí con
   `fs_write`/`root_exec` y el maestro (el Samsung) lo replica al resto de nodos. No
   hace falta acceso de escritura al repo.
3. **Si el agente externo no tiene MCP**: puede hablar HTTP con `curl` contra el mismo
   endpoint `/mcp` (JSON-RPC 2.0 crudo); `rust-core/tools/mdmcp.py` es sólo una envoltura.
4. **El `X-Seere-Auth-Token` sólo hace falta para escribir** (fs_write, POST). Para
   `tools/list` y las lecturas, basta la API key.
5. **Verificar que el prompt funcionó**: tras su primer mensaje debe haber aparecido
   `/data/local/tmp/megadaemon/state/agents/<nombre>.json` y al menos una línea en
   `sync/re_<nombre>.ndjson` y en `mesh_journal.ndjson`.
