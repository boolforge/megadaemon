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
| Dispositivo | Samsung S21 Ultra (Exynos 2100), root + Magisk — IP LAN **192.168.1.124** |
| Cockpit (web) | `http://localhost:3000` en el móvil · `http://192.168.1.124:3000` en la LAN |
| Alias de puertos | 3000, 3030, 8090, 8080 (puente local) + 9090 (cockpit nativo) |
| Túnel externo | `https://webpage-ultram-please-communicate.trycloudflare.com` |
| MCP (JSON-RPC 2.0) | `POST https://webpage-ultram-please-communicate.trycloudflare.com/mcp` (**no** `/sse`) |
| Cabeceras | `X-API-Key: <MEGADAEMON_API_KEY>` siempre · `X-Seere-Auth-Token: <MEGADAEMON_COCKPIT_TOKEN>` sólo al escribir |
| ADB | `adbd` en TCP **5555**: `adb connect 192.168.1.124:5555` (varios equipos a la vez) |
| ADB nativo Rust | `rust-core/src/adb_client.rs` (protocolo ADB v1 a mano, sin binario `adb`) |

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
Endpoint MCP (JSON-RPC 2.0 puro, POST):
    https://webpage-ultram-please-communicate.trycloudflare.com/mcp
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
  `adb connect 192.168.1.124:5555` desde cualquier PC de la red.
- **Persistencia**: `deploy/91-adb-tcp.sh` (Magisk `service.d`) deja ADB por
  TCP activo en cada arranque y lo levanta si `adbd` se cae.
  Para apagarlo: `sh /data/adb/service.d/91-adb-tcp.sh off`.
