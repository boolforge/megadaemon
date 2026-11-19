# Prompt de conexión para un AGENTE EXTERNO a la instancia MAESTRA de MegaDaemon

> Documento vivo. Todo lo de aquí está **verificado en vivo** contra el Samsung
> (20:5x del 19/11/2026): `GET /health` → `200 {"status":"ok","version":"13.3.0"}`,
> `POST /mcp` → `tools/list` con 25 herramientas.
>
> Copia el bloque de abajo y pégalo en el agente externo (cuenta de MiniMax,
> Claude, DeepSeek, el que sea). Está escrito para que el agente externo
> **no improvise nada**: usa la instancia, no la reescriba.

---

## 0. TL;DR — los 6 datos que necesitas

| Qué | Valor |
|---|---|
| **Instancia** | El **Samsung** es la instancia MAESTRA (`primary`). El PC Linux y los demás son `worker`. |
| **URL base (cockpit)** | `https://webpage-ultram-please-communicate.trycloudflare.com` |
| **MCP** | `POST https://webpage-ultram-please-communicate.trycloudflare.com/mcp` (JSON-RPC 2.0, **NO** `/sse`) |
| **Cabecera siempre** | `X-API-Key: ${MEGADAEMON_API_KEY}` (valor real cifrado en el almacen de secretos; NUNCA en el repo) |
| **Cabecera al escribir** | `X-Seere-Auth-Token: ${MEGADAEMON_COCKPIT_TOKEN}` (idem) |
| **En el propio móvil** | `http://localhost:3000` (alias `3000/3030/8080/8090` → `9090`) |

**Las credenciales NO están en este documento** (GitHub bloquea los pushes que
contienen tokens, y con razón): están **cifradas** en el almacén de secretos del
entorno (`MEGADAEMON_API_KEY` y `MEGADAEMON_COCKPIT_TOKEN`; en este entorno ya
creados) y, en el dispositivo, en `state/megadaemon_api.key` y
`state/cockpit_token` (0600). Para obtenerlas en el móvil:
`cat /data/user/0/com.agy/files/megadaemon/state/megadaemon_api.key` y
`cat /data/user/0/com.agy/files/megadaemon/state/cockpit_token` (o
`/data/local/tmp/megadaemon/megadaemon.env`).

---

## 1. QUÉ ES MEGADAEMON (para que el agente externo lo entienda)

MegaDaemon es un **harness de ingeniería inversa para Android escrito 100 % en Rust**
(sin Python, sin aapt, sin scripts externos) que corre **en el propio móvil con root**
y que, además, expone una API por HTTP para que **otros agentes** trabajen sobre el
mismo dispositivo.

- **Instancia MAESTRA** = el Samsung (rol `primary`): reparte trabajo, decide y
  sincroniza. Las demás instancias son `worker` y sólo ejecutan lo asignado.
- **Bus de trabajo** = cola persistente (SQLite) con estados
  `pending/claimed/running/done/failed`, `claimed_by` (id del agente),
  `lease_until` (recuperar tareas de un worker muerto) y `attempts` acotados.
  **La cola ES el protocolo**: el chat entre agentes es sólo para desbloqueos.
- **Estado compartido** = `<state_dir>/` con `sync/` (NDJSON), `instance.json`
  (id, rol, latido), `task_queue.json` y la tabla SQLite.
- **Capacidades** (vía MCP, 25 herramientas): control del dispositivo con root
  (`system_overview`, `root_exec`, `fs_read/write`, `find_duplicates`,
  `props_and_settings`, `selinux_control`, `magisk_module`, `kernel_and_blocks`,
  `app_control`, `screen_and_ui`), inventario y análisis de APKs (manifiesto
  AXML, permisos, componentes, DEX, `.so`, SHA-256, detección de deriva),
  RE (binwalk/DiE, DEX, descompilación, desofuscación), sincronización de
  conversaciones, orquestación (capacidades, cola, alertas, mesh) y **ADB remoto**.

---

## 2. CÓMO CONECTAR Y VERIFICAR (haz esto PRIMERO, antes de cualquier otra cosa)

```bash
BASE="https://webpage-ultram-please-communicate.trycloudflare.com"
KEY="${MEGADAEMON_API_KEY}"
TOK="${MEGADAEMON_COCKPIT_TOKEN}"

# 1) ¿está viva la instancia?
curl -s -H "X-API-Key: $KEY" "$BASE/health"      # -> {"requests":N,"status":"ok","uptime":S,"version":"13.3.0"}

# 2) catálogo real de herramientas (NUNCA asumas firmas)
curl -s -X POST "$BASE/mcp" -H "X-API-Key: $KEY" -H "X-Seere-Auth-Token: $TOK" \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'

# 3) una llamada de sólo lectura
curl -s -X POST "$BASE/mcp" -H "X-API-Key: $KEY" -H "X-Seere-Auth-Token: $TOK" \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":2,"method":"tools/call",
          "params":{"name":"system_overview","arguments":{}}}'
```

Reglas de la API:
- `X-API-Key` **siempre**; `X-Seere-Auth-Token` **sólo** si modifica estado.
- Todo lo que no sea `GET /`, `/health` y `/metrics` exige token → **fail-closed**.
- `GET /` sin `Origin` foráneo devuelve la UI con `window.MEGADAEMON_TOKEN` inyectado.
- Toda acción irreversible exige `confirm: true`. El daemon es root: **no la uses
  para tocar cosas sin permiso explícito del usuario**.

---

## 3. PROTOCOLO DE TRABAJO: PRIMERO VOLCA TUS DATOS, LUEGO AYUDAS

El usuario lo pidió así de explícito: el agente externo debe **aportar sus datos**
al almacén común para facilitar investigaciones futuras, y sólo después ayudar.

### FASE A — Volcado completo de TUS datos (obligatorio, antes de proponer nada)

1. **Inventario** de lo tuyo: qué has researched, qué ficheros has creado, qué
   datasets, qué scripts, qué findings. Formato: una tabla
   `artefacto | ruta | formato | tamaño | sha256 | qué aporta`.
2. **Volcado al almacén compartido** de la instancia:
   ```bash
   # subir tus datasets al estado compartido de la instancia (vía MCP: fs_write,
   # o directamente al repositorio de datos compartido si lo tienes)
   curl -s -X POST "$BASE/mcp" -H "X-API-Key: $KEY" -H "X-Seere-Auth-Token: $TOK" \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":3,"method":"tools/call",
             "params":{"name":"fs_write","arguments":{"path":"<state>/sync/externo/<fichero>","content":"<contenido>","base64":false}}}'
   ```
   Convenciones: `sync/externo/<agente>/<dataset>.ndjson` (NDJSON, una línea por
   hallazgo, con `source`, `ts`, `confidence`, `evidence`), y `sync/externo/<agente>/README.md`
   con el índice de lo aportado.
3. **Manifiesto de lo aportado** con SHA-256 de cada fichero (para que otro agente
   pueda verificar que no cambió).
4. **Reclamo en la cola** para que el trabajo sea visible y no se duplique:
   `md-coord add "<qué haces>" --priority 3 --owner <tu-agente>` → `claim` → `report`.

### FASE B — Evalúa y-propón (sólo después de la Fase A)

- ¿Qué falta en el producto? Prioriza por impacto × esfuerzo.
- Propón el trabajo como una **tarea de la cola**, no como un chat.
- Si tocas código del repo: un módulo = un agente. Avisa por el tablón
  (`docs/COORDINATION.md`) antes de tocar un módulo ajeno.

### FASE C — Ejecuta y reporta

- Reporta en **3 líneas**: qué hiciste, qué tocas, qué bloquea.
- Nunca `git add -A`; nunca `--force`; si el push no es fast-forward, rebase o aborta.
- Token de GitHub: en el entorno, secreto `GITHUB_TOKEN` (nunca en ficheros).

---

## 4. ADB (acceso al móvil sin depender de la IP de LAN)

- Cliente ADB **en Rust puro**: `rust-core/src/adb_client.rs` (protocolo
  CNXN/OPEN/WRTE/OKAY/CLSE, CRC-32 propio, 9 tests).
- **Clave RSA-2048 de host** generada DENTRO del Samsung con Python puro
  (Miller-Rabin): privada en `/data/local/tmp/megadaemon/adb_host_key.pem` (0600,
  root), pública en `/data/misc/adb/adb_keys` como `mavis@samsung`.
  **Por qué**: `adbd` exige que la clave pública del cliente esté autorizada;
  sin ella, `adb connect` responde `unauthorized`. Es el mismo mecanismo que
  `~/.android/adbkey` en un PC. La clave privada **nunca salió del móvil**.
- **Camino robusto e inmune al bloqueo por IP**: el daemon expone ADB como
  *gateway HTTP* (`POST /api/adb/shell`) y todo el tráfico va por el túnel
  Cloudflare → funciona aunque la IP del móvil cambie o el router bloquee.
- `adb_status()` / `remote_adb()` en `repo_updates.rs` para activar ADB sobre TCP.

---

## 5. EFICIENCIA (el usuario lo pide explícitamente)

- En el móvil, el consumo de MegaDaemon es **8,9 MB RSS**; la saturación del
  Samsung (10,2/10,5 GB, 3,3 GB de swap, 78 % iow) la causan **las apps de
  Android** (system_server, Chrome, GMS…), no MegaDaemon.
- En el sandbox: se contaron **52 procesos `cargo` simultáneos** sobre un NAS de
  3 GB → OOM (load 31) y errores **falsos** de compilación
  (`failed to copy bitcode`, `rust-lld: cannot open ...rcgu.o`, `database is locked`).
  Solución ya en el repo: `tools/build-queue.sh` (cola con `flock`, 2 slots,
  target compartido, `CARGO_INCREMENTAL=0`, `-j 2`, guardia de memoria).
  **Úsalo siempre**: `tools/build-queue.sh build --release` / `test --release`.
- No reimplementes lo que ya está: `zip 2.2` (features deflate) + `flate2` para
  ZIP/DEFLATE, `sha2` para hashes, `rusqlite` para estado, `ureq` para HTTP,
  `serde_json` para datos. **Un ZIP propio duplica bugs y quema tokens.**

---

## 6. REPO

- `https://github.com/boolforge/megadaemon` (rama `main`).
- Tablón de coordinación (estado compartido, una línea por agente):
  `docs/COORDINATION.md`. Plan maestro: `docs/MASTER_PLAN.md`.
- Documentos clave: `docs/DEVICE_DEPLOY.md`, `docs/CONNECT_EXTERNAL_AGENT.md`,
  `docs/AUDIT_V14_UI_HTTP.md`, `docs/ROADMAP_V14.md`.

---

## 7. EL PROMPT PARA PEGAR (copia desde aquí)

```
Tienes acceso a MEGADAEMON, un harness de ingeniería inversa para Android
escrito 100 % en Rust que corre CON ROOT en el móvil del usuario (Samsung),
que es la instancia MAESTRA (primary). Tu trabajo es USARLO y APORTAR, no
reimplementarlo ni reinventarlo.

CONEXIÓN
  BASE  = https://webpage-ultram-please-communicate.trycloudflare.com
  MCP   = POST $BASE/mcp   (JSON-RPC 2.0; NO /sse)
  Headers: X-API-Key: ${MEGADAEMON_API_KEY}
           X-Seere-Auth-Token: ${MEGADAEMON_COCKPIT_TOKEN}  (sólo al escribir)
  PRIMERO: GET $BASE/health  y luego POST $BASE/mcp {"method":"tools/list"}.
  No asumas firmas de API: lee el catálogo real (25 herramientas).

FASE A — VOLCA TUS DATOS PRIMERO (obligatorio antes de proponer nada)
  1) Inventario de TODO lo tuyo: dataset | ruta | formato | tamaño | sha256 | qué aporta.
  2) Súbelo al almacén compartido vía MCP (fs_write) a
     sync/externo/<tu-agente>/<dataset>.ndjson  (una línea por hallazgo con
     source, ts, confidence, evidence) y un sync/externo/<tu-agente>/README.md
     como índice.
  3) Manifiesto con el SHA-256 de cada fichero subido (verificable por otros).
  4) Reclamo la tarea en la cola (md-coord add/claim) para que no se duplique.

FASE B — EVALÚA Y PROPÓN
  ¿Qué falta en MegaDaemon? Prioriza impacto × esfuerzo. Cada propuesta es una
  TAREA DE LA COLA, no un mensaje. Antes de tocar el módulo de otro agente,
  avisa en el tablón docs/COORDINATION.md.

FASE C — EJECUTA Y REPORTA EN 3 LÍNEAS
  qué hiciste / qué tocas / qué bloquea. Nada de mensajes largos.
  Git: un commit por módulo, `git add <ruta exacta>` (NUNCA -A), nada de
  --force; si el push no es fast-forward, rebase o aborta. Token: secreto
  GITHUB_TOKEN del entorno (nunca en ficheros).

REGLAS DURAS
  - El daemon es ROOT: toda acción irreversible exige confirm=true; no toques
    material del usuario sin permiso explícito.
  - No reimplementes: zip 2.2 + flate2 (ZIP/DEFLATE), sha2 (hashes), rusqlite
    (estado), ureq (HTTP), serde_json (datos).
  - Builds: tools/build-queue.sh build|test --release (cola con flock; el target
    compartido se corrompe con builds concurrentes: 52 cargo simultáneos ->
    OOM y errores falsos de compilación).
  - Si algo falla, MEJÓRALO en tu módulo y repórtalo en 3 líneas.
```
