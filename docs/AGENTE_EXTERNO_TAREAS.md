# Prompt de tareas para un AGENTE EXTERNO que se conecta a la instancia MAESTRA (Samsung)

> Copia el bloque **PROMPT** de abajo y pégaselo tal cual al agente externo.
> **Este fichero NO contiene secretos.** Las credenciales viven fuera del repo,
> en `/workspace/md-secrets/instance.env` (0600, dir 0700) y en el móvil en
> `/data/local/tmp/megadaemon/state/` (`megadaemon_api.key`, `cockpit_token`).

---

## Datos de conexión (para pegar en el prompt)

| Dato | Valor |
|---|---|
| Rol de la instancia | `primary` (maestra) — decide, reparte y sincroniza |
| Dispositivo | Samsung S21 Ultra (Exynos 2100), root + Magisk — IP LAN **<IP_DEL_MOVIL>** |
| Cockpit (web) | `http://localhost:3000` en el móvil · **`http://<IP_DEL_MOVIL>:3000` en la LAN** (RUTA CANÓNICA: es estable) |
| Alias de puertos | 3000, 3030, 8090, 8080 (puente local) + 9090 (cockpit nativo) |
| MCP por LAN | **`POST http://<IP_DEL_MOVIL>:3000/mcp`** (mismo daemon, mismo JSON-RPC 2.0; **no** `/sse`) |
| Túnel externo | **EFÍMERO**: la URL vigente está en `<state_dir>/tunnel.url` en el móvil (`/data/local/tmp/megadaemon/tunnel.url`) y cambia cada vez que cloudflared se relanza (⇒ 530). Última conocida: `https://TUNEL.trycloudflare.com` — **úsala sólo si la LAN falla**, y entonces léela del fichero, no la copies de aquí. |
| MCP (JSON-RPC 2.0) | `POST <URL del túnel>/mcp` (**no** `/sse`) · cabeceras: `X-API-Key` siempre · `X-Seere-Auth-Token` sólo al escribir |
| Credenciales | `/workspace/md-secrets/instance.env` (0600) — **nunca al chat**. `tools/phone_url.py --export` refresca las URLs (las del túnel caducan en minutos) |
| ADB | `adbd` en TCP **5555**: `adb connect <IP_DEL_MOVIL>:5555` (varios equipos a la vez) |
| ADB nativo Rust | `rust-core/src/adb_client.rs` (protocolo ADB v1 a mano, sin binario `adb`) |

---

## PRIMER ENCARGO CONCRETO (lo primero que debe hacer el nodo externo)

> No le digas "lee el backlog y ayuda": se aburre. Dale estos tres encargos,
> en este orden, con entregable concreto en cada uno. Son del área del maestro
> (Samsung) y son justos los que el enjambre no puede resolver desde dentro.

**ENCARGO 1 — Validar el fix del 502 del maestro (tú estás fuera: vantage point ideal)**
El maestro devuelve 502 de forma intermitente. Causa raíz ya diagnosticada:
`ensure_live_shm()` entra en pánico mientras se mantiene el `Mutex<GlobalCockpitState>`
global del cockpit → ese panic **envenena el mutex para siempre** → los `lock().unwrap()`
muerden otra vez → sin `catch_unwind` la conexión se corta a medias → Cloudflare ve
respuesta incompleta → 502. El watchdog sólo miraba `pgrep` (proceso vivo) → nunca reiniciaba.
Entregable: **20 peticiones seguidas** al endpoint del maestro con las cabeceras de
conexión, mediendo código y latencia, y un veredicto: ¿502 estable, intermitente o cero?
Si aparece un 502, captura el cuerpo exacto y el `logcat` del móvil (`logcat -d -t 200 | grep -i adbd`)
en el mismo instante. Sin ese par de datos no se arregla nada.

**ENCARGO 2 — Cerrar el handshake ADB contra el Samsung**
En el Samsung, `adbd` escucha en TCP 5555 (`<IP_DEL_MOVIL>:5555`) y el socket **acepta**
la conexión, pero `adbd` responde **RST** en cuanto recibe el CNXN
(logcat: `host-16: connection terminated: read failed`). Pistas ya reunidas:
`ro.adb.secure=0`; la clave de host **ya está registrada** en
`/data/misc/adb/adb_keys` como `mavis@fleet`; el cliente es
`rust-core/src/adb_client.rs` (Rust puro, sin binario `adb`) y hay un binario ARM64
precompilado en `dist/adbonly-arm64` (con su `.sha256`).
Entregable: el handshake completo hasta obtener el **banner** de `adbd`, o el
informe exacto de en qué paso se rompe (con los bytes que se enviaron y los que
se recibieron) y el parche propuesto en `adb_client.rs`.

**ENCARGO 3 — RE del Exynos 2100 (tu especialidad)**
Volcado RE completo del SoC: `sysfs`, `/proc`, registros del bootloader,
tablas de firmware, y **por qué `ensure_live_shm()` entra en pánico** (`src/shm.rs`):
qué es ese shm, quién lo crea, por qué se envenena y cuál es el diseño correcto
(crear el shm antes de tomar el lock; nuncaargestar bajo el mutex global).
Entregable: informe con el diff propuesto para `shm.rs` + `cockpit_server.rs`,
con el test que demuestra que un panic en `ensure_live_shm()` ya no tumba el servidor.

---

## PROMPT (copiar desde aquí)

```text
# ENCARGO: NODO EXTERNO del enjambre MegaDaemon — conecta, vuelca tus datos y luego ayudas

## 0. Quién eres y qué es esto
Eres un NODO EXTERNO de MegaDaemon. La instancia MAESTRA es un Samsung S21 Ultra
(Exynos 2100, root + Magisk) que hace de coordinator: reparte trabajo, guarda el
estado compartido y decide. Tú entras como nodo WORKER: aportas tus datos y tu
músculo. No eres el maestro y no se toca su configuración sin acuerdo.

## 1. CONECIÓN (primero, sin preguntar nada)
Endpoint MCP (JSON-RPC 2.0 puro, POST) — **por la LAN primero (es estable)**:
    http://<IP_DEL_MOVIL>:3000/mcp
Si la LAN no responde, usa el túnel (URL EFÍMERA: léela del móvil en
/data/local/tmp/megadaemon/tunnel.url; no la copies de un prompt viejo):
    POST https://<hostname-vivo>.trycloudflare.com/mcp
Cabeceras obligatorias en CADA POST:
    X-API-Key: <PEGAR_AQUI: MEGADAEMON_API_KEY>
    X-Seere-Auth-Token: <PEGAR_AQUI: MEGADAEMON_COCKPIT_TOKEN>   (sólo si vas a escribir)
    Content-Type: application/json
Body de ejemplo:
    {"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
    {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"system_overview","arguments":{}}}
Cliente ya hecho (repo): rust-core/tools/mdmcp.py
    python3 mdmcp.py tools ; python3 mdmcp.py call system_overview '{}'
PRIMER MOVIMIENTO OBLIGATORIO: `tools/list` COMPLETO + `system_overview`.
Muéstrame ambos resultados antes de tocar nada.

TUS TRES ENCARGOS (no te pregunten qué hacer; están definidos arriba):
  E1 validar el fix del 502 desde fuera · E2 cerrar el handshake ADB · E3 RE del Exynos.

## 2. FASE 1 — VOLCADO COMPLETO DE TUS DATOS (antes de cualquier otra cosa)
Antes de ayudar en tareas, el maestro necesita TODO lo que tienes. Vuelca, en
este orden, y anuncia cada entrega:
  2.1 `system_overview` + `props_and_settings` (action=read) → inventario del sistema.
  2.2 `app_control` (action=list) + `apk_inventory` → apps, versiones y hashes.
  2.3 `fs_read` de tus datasets (inventarios, logs, datasets de RE, capturas de
      pantalla, etc.): volcado ÍNTEGRO,Chunk a chunk si hace falta, sin resumir.
  2.4 `sync_conversations` + `conversation_stats` + `search_conversations` para
      el histórico completo (con cookies sólo si es imprescindible).
  2.5 Tu informe de estado: qué has volcado, tamaños y hashes (SHA-256) de cada
      dataset, y qué NO has podido traer (con el motivo).
Todo lo que subas debe ir con su SHA-256: el maestro lo verifica antes de
aceptarlo. Si algo falla al subirlo, NO lo borres y NO lo maquilles: dilo.

## 3. FASE 2 — AYUDAR (con lo que te queda de tokens)
Sólo cuando el volcado esté completo y anunciado:
  3.1 Lee el tablón y el backlog: `docs/MASTER_PLAN.md`, `docs/COORDINATION.md`.
  3.2 Reclama una tarea concreta por su id/ámbito y dilo (no trabajes en algo
      que otro nodo tenga reclamado).
  3.3 Ejecuta, verifica y reporta en el formato de 3 líneas del tablón:
      (1) qué hiciste  (2) qué tocas/falta  (3) qué te bloquea.
  3.4 Si te quedas sin tokens, devuelve el control con el estado limpio y un
      informe final: lo entregado, lo pendiente y lo que sigue.

## 4. REGLAS (no negociables)
- Nada de `git add -A` ni de reescribir historia: `git add <ruta exacta>`.
- Un nodo = un módulo/fichero. Si necesitas tocar otro, avisa al maestro ANTES.
- Reutiliza lo que ya existe: `zip 2.2` + `flate2` (ZIP/DEFLATE), `sha2`,
  `rusqlite`, `ureq`, `serde_json`. NO reimplementes ZIP ni hashes.
- Toda acción irreversible (instalar, escribir en /system, borrar) exige
  `confirm: true` explícito. Nada de destructivo "para ir rápido".
- Nada de panic ni esperas infinitas: todo con timeout y con tope de salida.
- Secretos: nunca al repo, nunca en un mensaje de informe, nunca en logs.
- Si algo falla, MEJORA el módulo del que es dueño y dilo. No lo parchees a
  escondidas en el fichero de otro.

## 5. ENTREGABLE FINAL
Un volcado completo con hashes + el informe de 3 líneas por tarea + la lista
exacta de lo que queda pendiente y de qué necesita el maestro.
```

---

## Notas para el usuario

- **Credenciales**: `cat /workspace/md-secrets/instance.env` (en el móvil:
  `/data/local/tmp/megadaemon/state/megadaemon_api.key` y `cockpit_token`).
  No están en el repo a propósito: el repo es público.
- **ADB**: el móvil tiene `adbd` en TCP 5555 y un cliente ADB en Rust puro
  (`rust-core/src/adb_client.rs`), sin depender del binario `adb`.
  `adb connect <IP_DEL_MOVIL>:5555` desde cualquier PC de la red.
- **Persistencia**: `deploy/91-adb-tcp.sh` (Magisk `service.d`) deja ADB por
  TCP activo en cada arranque y lo levanta si `adbd` se cae.
  Para apagarlo: `sh /data/adb/service.d/91-adb-tcp.sh off`.
