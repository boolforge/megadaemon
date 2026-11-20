# Conexión de un agente externo a la instancia MAESTRA (Samsung)

> Documento **sin secretos**. Las credenciales viven en ficheros `0600` fuera
> del repo (`/workspace/md-secrets/`), nunca en el chat ni en el control de
> versiones. Este doc explica *qué* es MegaDaemon, *cómo* conectarse y *qué*
> hay que devolver para que el enjambre se beneficie.

## 0. TL;DR para el agente externo

```
Túnel      : https://webpage-ultram-please-communicate.trycloudflare.com
MCP        : POST https://webpage-ultram-please-communicate.trycloudflare.com/mcp
             (JSON-RPC 2.0 — NO /sse, NO GET)
Cabeceras  : X-API-Key: <MEGADAEMON_API_KEY>
             X-Seere-Auth-Token: <MEGADAEMON_COCKPIT_TOKEN>
Web (LAN)  : http://192.168.1.124:3000  (y :3030, :8090, :9090)
Credenciales: /workspace/md-secrets/instance.env  (0600) — cópialas a tu entorno,
              no las pegues en ningún sitio que se guarde.
```

Si prefieres MCP por HTTP directo al móvil (sin pasar por el túnel), usa
`http://192.168.1.124:9090/mcp` con las mismas dos cabeceras.

## 1. Qué es MegaDaemon (para quien llega nuevo)

MegaDaemon es un **agente RE/Android en Rust** que vive **en el propio
dispositivo** y expone, por HTTP, todo lo que el móvil sabe de sí:

| Capa | Qué es | Módulo |
|---|---|---|
| Cockpit | servidor HTTP + UI web (HTML embebido en el binario) | `rust-core/src/cockpit_server.rs` |
| MCP | servidor JSON-RPC 2.0 con 26 herramientas (root_exec, fs_read/write, apps, pantalla, Magisk, SELinux, DEX/Rizin/DiE, NPU…) | `rust-core/src/mcp_server.rs` |
| SEERE | motor de razonamiento propio (gramática GBNF, taint, acciones) | `rust-core/src/seere_v11.rs` |
| Nube | catálogo de proveedores + enjambre | `rust-core/src/providers.rs`, `src/coordination.rs` |

Topología: hay **varias instancias** (el Samsung es la **MAESTRA / `role: primary`**;
los PC Linux del enjambre son *workers*). La instancia maestra decide, reparte
tareas por el tablero (`state/board.sqlite`, CLI `md-coord`) y sincroniza
instantáneas con `state/sync/*.ndjson`.

La instancia se identifica por `<STATE_DIR>/instance.json`
(`name`, `role`, `device`, `model`…). La del móvil está en
`/data/local/tmp/megadaemon/state/instance.json`.

## 2. Conectarse (tres caminos, todos con la misma identidad)

### 2.1 MCP por el túnel (recomendado desde fuera de la LAN del móvil)

```bash
set -a; . /workspace/md-secrets/instance.env; set +a
curl -s -X POST "$MEGADAEMON_MCP" \
  -H "X-API-Key: $MEGADAEMON_API_KEY" \
  -H "X-Seere-Auth-Token: $MEGADAEMON_COCKPIT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

Salida esperada: 26 herramientas. Ejemplo útil:

```bash
curl -s -X POST "$MEGADAEMON_MCP" \
  -H "X-API-Key: $MEGADAEMON_API_KEY" -H "X-Seere-Auth-Token: $MEGADAEMON_COCKPIT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"root_exec","arguments":{"command":"getprop ro.product.model; free -m | head -2"}}}'
```

### 2.2 Web del móvil (navegador)

* **Desde el PC del usuario, en la misma LAN**: <http://192.168.1.124:3000>
  (también `:3030`, `:8090`, `:9090`).
* **Desde el propio móvil**: `http://127.0.0.1:3000`.
* **Desde fuera de la LAN**: la URL del túnel; la raíz `/` está **exenta** del
  gate de API key (arranque v13.4), el resto de rutas sí lo exige.

> **Por qué `localhost` daba `ERR_CONNECTION_REFUSED`** (las 4 capturas del
> usuario): `localhost` en el **PC del usuario** es el PC, no el móvil. El
> móvil servía el cockpit **sólo en `127.0.0.1`**, así que desde la LAN no
> había ruta. Desde v13.4 el cockpit escucha en **`0.0.0.0`** y expone los
> puertos alias (3000/3030/8090/9090) él mismo, sin depender del puente
> Python.

### 2.3 ADB (control direto del dispositivo)

* `adbd` del Samsung escucha en **TCP 5555** (`*:*:5555`).
* **La clave RSA**: un agente Generó un par RSA (`adb_host_key.pem` +
  `adb_host_key.pub`, en `/workspace/md-secrets/`) y lo autorizó en el
  teléfono; **`adbd` rechaza las sesiones que no/key no coincida** (por eso el
  móvil aparecía como `offline` en `adb devices`). No es un bloqueo del
  usuario: es el mecanismo normal de autenticación de ADB, y la clave está
  bajo control del usuario. Copia la clave a la ruta estándar antes de usar
  el cliente:
  ```bash
  mkdir -p ~/.android && cp -f /workspace/md-secrets/adb_host_key.pem ~/.android/adbkey && chmod 600 ~/.android/adbkey
  adb kill-server && adb start-server && adb connect 192.168.1.124:5555
  ```
* Cliente ADB **en Rust puro** (sin binario `adb`): `rust-core/src/adb_client.rs`
  — habla el protocolo v1 por socket contra `adbd` (CNXN/OPEN/WRTE), con
  tope de bytes y timeouts.
* CLI de operación: `rust-core/src/bin/megadaemon-adb.rs`
  (`devices | doctor | deploy | health | logs`).

## 3. Cómo devolver tu trabajo al enjambre (volcar tus datos)

1. **Tablero (obligatorio)**: toda tarea se reclama y se reporta en 3 líneas
   (qué / qué tocas / qué bloquea):
   ```bash
   md-coord add "mi tarea" --priority 2 --owner <mi-agent>
   md-coord claim <id> --owner <mi-agent>
   md-coord report <id> --owner <mi-agent> --ok "qué/toque/bloquea"
   ```
2. **Código**: un fichero = un agente. `git add <ruta exacta>` (nunca `-A`),
   commit con **solo tus ficheros**, push a `https://github.com/boolforge/megadaemon`.
3. **Datos al enjambre**: instantáneas en `state/sync/<agent>.ndjson` (formato
   una línea JSON por elemento) — el maestro las fusiona. Si necesitas bulky
   (dumps, capturas), guárdalos en `state/sync/` y deja **sólo el puntero**
   en el ndjson.
4. **Publica el resultado en el chat de la sesión** aunque no haya nada
   nuevo: el usuario haExplicitado que los agentes "no comunican" y eso le
   bloquea. Formato corto: 3 líneas.

## 4. Reglas de convivencia (no negociables)

* **El móvil va justo de RAM** (10,2 GB de 10,5 GB usados, 3,3 GB de swap, load
  ~20 por apps de Android). Nada de procesos extra, nada de polling agresivo,
  nada de ráfagas largas: **el daemon debe pesar ~8,5 MB y 4-21 hilos**.
* **No reimplementar** ZIP/deflate/hash/HTTP: las crates (`zip 2.2`,
  `flate2`, `sha2`, `rusqlite`, `ureq`, `serde_json`) ya están en el repo.
* **No tocar** ficheros de otro agente sin avisar antes al principal
  (las colisiones ya han pasado varias veces).
* **No tocar** `rust-core/src/apk_inventory.rs` (dueño: sesión del inventario).
* **Secretos**: se leen de ficheros `0600`, nunca se pegan en conversaciones
  (el historial es público para quien lo lea).

## 5. Prompt listo para pegar a un agente externo

```
Eres un agente que se conecta a la instancia MAESTRA de MegaDaemon (un
agente RE/Android en Rust que vive en el Samsung del usuario y se expone
por HTTP/MCP). Tu trabajo es conectarte, entender qué ofrece y devolver
tus hallazgos al enjambre.

CÓMO CONECTARTE
- MCP: POST https://webpage-ultram-please-communicate.trycloudflare.com/mcp
  (JSON-RPC 2.0; NO uses /sse ni GET).
- Cabeceras obligatorias en TODAS las llamadas:
    X-API-Key: <MEGADAEMON_API_KEY>
    X-Seere-Auth-Token: <MEGADAEMON_COCKPIT_TOKEN>
- Las credenciales están en /workspace/md-secrets/instance.env (0600): cógelas
  de ahí y exporta las variables; NO las pegues en el chat, ni en el repo, ni
  en ficheros versionados.
- Web del móvil (UI completa): http://192.168.1.124:3000 desde la LAN.
- ADB (control directo): adbd en 192.168.1.124:5555. La clave RSA del host
  está en /workspace/md-secrets/adb_host_key.pem (cópiala a
  ~/.android/adbkey, chmod 600). Cliente ADB en Rust puro:
  rust-core/src/adb_client.rs; CLI: rust-core/src/bin/megadaemon-adb.rs
  (devices | doctor | deploy | health | logs).

QUÉ HAY AHÍ (26 herramientas MCP)
root_exec, fs_read, fs_write, props_and_settings, selinux_control,
magisk_module, kernel_and_blocks, app_control, screen_and_ui,
system_overview, find_duplicates, conversation_stats, list_conversations,
search_conversations, get_conversation, sync_conversations,
sync_web_conversations, harvest_sessions, ingest_export, plus el motor RE
(megadaemon_die_binwalk, megadaemon_rizin, megadaemon_cirx_analyze,
megadaemon_mba_simplify, megadaemon_npu_cv) y el enjambre (minimax_chat,
swarm_chat).
Empieza por tools/list y luego llama a lo que necesites. root_exec corre como
root real (su -mm -c) con timeout y tope de salida.

QUÉ DEVOLVER (obligatorio)
1. Un informe corto con lo que encontraste y qué recommendation es.
2. Los datos crudos: en state/sync/<tu-agente>.ndjson (una línea JSON por
   elemento) y, si son bulky, los ficheros en state/sync/ con sólo el puntero
   en el ndjson.
3. Tu estado en el tablero: md-coord add/claim y md-coord report <id>
   --owner <tu-agente> --ok "qué/toque/bloquea" (3 líneas).
4. Código: sólo si es tuyo; git add <ruta exacta> (nunca -A), commit aislado
   y push a https://github.com/boolforge/megadaemon.
5. Publica el resultado en el chat aunque sea para decir "nada nuevo": la
   comunicación es obligatoria.

LÍMITES
- El móvil va justo de RAM (10,2/10,5 GB): nada de procesos extra ni polling
  agresivo.
- No reimplementes ZIP/deflate/hash/HTTP (crates ya presentes).
- No toques ficheros de otro agente sin avisar al principal.
- No pegues secretos en el chat.
```

## 6. Estado a 2026-11-20 (medido, no supuesto)

| Comprobación | Resultado |
|---|---|
| `/health` por el túnel | **200** — `{"status":"ok","version":"13.4.0",...}` |
| `/health` en el móvil (`127.0.0.1:9090`) | **200** |
| Web en LAN (`http://192.168.1.124:3000`) | **200** + HTML de la UI |
| Bind del cockpit | `0.0.0.0` (LAN + túnel + loopback) |
| Puertos alias | 3000, 3030, 8090, 9090 |
| MCP `initialize` | OK, 26 herramientas |
| Coste por petición | ~1,5 ms de CPU en 9090 directo (medido, 200 peticiones) |
| RSS del daemon | ~8,4 MB (VmHWM 12 MB), 21 hilos |
| `adbd` | escuchando en `*:5555`, autenticado por clave RSA del host |
