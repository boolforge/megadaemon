# Conectar un agente externo a la instancia PRINCIPAL (Samsung)

> Documento operativo. **No contiene secretos**: las claves viven fuera del repo,
> en `/workspace/md-secrets/instance.env` (0600, dir 0700).

---

## 1. Identidad de la instancia

| Dato | Valor |
|---|---|
| Rol | `primary` (maestro) — decide, reparte y sincroniza |
| Dispositivo | Samsung (`samsung`), IP LAN **192.168.1.124** |
| Instancia | `MEGADAEMON_INSTANCE_ROLE=primary`, `MEGADAEMON_INSTANCE_NAME=samsung` |
| Web (cockpit) | `http://localhost:3000` en el móvil · `http://192.168.1.124:3000` desde la LAN |
| Alias de puertos | 3000, 3030, 8090, 8080 (puente local) + 9090 (cockpit real) |
| Túnel externo | `https://webpage-ultram-please-communicate.trycloudflare.com` |
| MCP (JSON-RPC 2.0) | `POST https://webpage-ultram-please-communicate.trycloudflare.com/mcp` (**no** `/sse`) |
| ADB | `adbd` escucha en **TCP 5555** en la LAN (`adb connect 192.168.1.124:5555`) |

## 2. Credenciales (dónde están y cómo usarlas)

Las tres sesiones de este workspace comparten sistema de ficheros, así que
**cualquier agente de la flota las lee localmente**:

```sh
cat /workspace/md-secrets/instance.env
# MEGADAEMON_API_KEY=mdaemon_sk_live_...
# MEGADAEMON_COCKPIT_TOKEN=...
# MEGADAEMON_URL=https://webpage-ultram-please-communicate.trycloudflare.com
# MEGADAEMON_MCP=https://webpage-ultram-please-communicate.trycloudflare.com/mcp
```

Cabeceras obligatorias en **cada** llamada:

```
X-API-Key: <MEGADAEMON_API_KEY>            # siempre
X-Seere-Auth-Token: <MEGADAEMON_COCKPIT_TOKEN>   # sólo si modifica estado (POST/PUT)
```

Ejemplo mínimo:

```sh
curl -s -H "X-API-Key: $MEGADAEMON_API_KEY" "$MEGADAEMON_URL/health"
curl -s -H "X-API-Key: $MEGADAEMON_API_KEY" \
     -H "X-Seere-Auth-Token: $MEGADAEMON_COCKPIT_TOKEN" \
     "$MEGADAEMON_URL/api/status"
```

**Nunca** escribas estos valores en ficheros del repo, en logs ni en commits.

## 3. Prompt listo para pegar al agente externo

```
Tienes acceso a una instancia de MegaDaemon (Rust puro, 100% sin Java) que YA
está funcionando y que actúa como harness de ingeniería inversa para Android.
No reimplements nada: USA la instancia.

═══════════════════════════════════════════════════════════════════
1. CONEXIÓN
═══════════════════════════════════════════════════════════════════
Instancia MAESTRA (primary) = el móvil Samsung, la que reparte tareas.
  URL base : https://webpage-ultram-please-communicate.trycloudflare.com
  MCP      : POST https://webpage-ultram-please-communicate.trycloudflare.com/mcp
             (JSON-RPC 2.0; usa /mcp, NO /sse)
  Cabeceras: X-API-Key: <API_KEY>                (siempre)
             X-Seere-Auth-Token: <COCKPIT_TOKEN> (sólo POST/PUT)
  Las dos claves te las da quien te asigna la tarea (nunca las busques en el repo).

═══════════════════════════════════════════════════════════════════
2. FASE 1 — VOLCADO COMPLETO DE DATOS (lo primero, antes de nada)
═══════════════════════════════════════════════════════════════════
Tu primera tarea es DESPERTAR y VACIAR el store común: la instancia maestra no
tiene datos tuyos y no puede ver lo que tú sabes.

  a) Descubre qué expone la instancia. Usa las tools MCP (tools/list) y las
     rutas del cockpit (/health, /api/status, /api/providers, /api/tasks, ...)
     para construir el INVENTARIO completo de capacidades.
  b) Vuelca TODO lo que tengas, sin filtrar por tamaño ni por urgencia:
       - APK/DEX de cada app instalada (hash sha256, package, versión, firma)
       - Metadatos de las apps, permisos, activities, receivers, providers
       - Inventario de hardware (SoC, NPU, sensores, particiones, DRAM)
       - Bases de datos SQLite de las apps que tengas acceso
       - Logs, cualquier artefacto binario o config que tengas en caché
  c) Cada artefacto va al almacén común de la instancia (tool MCP de escritura
     / endpoint del cockpit) con: nombre, origen, tamaño, sha256 y resumen.
  d) Al terminar, devuelve el ÍNDICE de lo volcado: cuántos artefactos, de qué
     tipos, y qué puntos quedan cubiertos.

═══════════════════════════════════════════════════════════════════
3. FASE 2 — SI QUEDAN TOKENS O TIEMPO, AYUDA
═══════════════════════════════════════════════════════════════════
  - Si la Fase 1 deja tokens: prioriza por el backlog de tareas que te pase la
    instancia maestra (tool MCP de tareas) y sigue vertiendo datos.
  - Si ves capability gaps en el inventario: propón el módulo nuevo con su
    contrato (tool MCP + endpoint), sin reescribir lo que ya funciona.
  - Nunca borres ni sobrescribas artefactos ya volcados por otro agente sin
    marcar la versión (sha256) en el registro.

═══════════════════════════════════════════════════════════════════
4. REGLAS
═══════════════════════════════════════════════════════════════════
  - Eres `worker`; la instancia Samsung es `primary` y decide ella.
  - No escribas claves, tokens ni claves privadas ADB en el repo.
  - Todo lo que produzcas debe acabar en el almacén común, no sólo en tu chat.
```

## 4. ADB contra el Samsung (sin binario `adb` en el móvil)

`adbd` ya expone **TCP 5555** en `192.168.1.124`, pero exige firma RSA
(`ro.adb.secure=1`) y `/data/misc/adb/adb_keys` estaba **vacío**: sin clave de
host registrada, `adb connect` se queda en `unauthorized`.

Estado y plan:

| Pieza | Estado |
|---|---|
| `adbd` en TCP 5555 | activo (`service.adb.tcp.port=5555`, `init.svc.adbd=running`) |
| Cliente ADB en Rust puro | `rust-core/src/adb_client.rs` (protocolo CNXN/OPEN/WRTE/OKAY/CLSE, sin binario externo) |
| Clave de host registrada | clave RSA de host instalada en `/data/misc/adb/adb_keys` (formato `usuario@host <base64>`) |
| Clave privada | sólo en `/workspace/md-secrets/` (fuera del repo), 0600 |
| Filtro de red | `iptables` restringiendo 5555 al rango de la LAN (no abierto a Internet) |

Desde el PC del usuario:

```sh
adb connect 192.168.1.124:5555
adb -s 192.168.1.124:5555 shell id     # => root (adbd en modo root)
```

Desde la flota (agentes Rust), el cliente puro:
`rust-core/src/adb_client.rs` → `Adbd::shell("id", ...)` / `AdbServer::devices()`.

## 5. Estado de la web del móvil

| Comprobación | Resultado |
|---|---|
| `GET http://localhost:3000/` | `200 OK`, UI real (89.707 B) |
| `GET /api/ping` | `{"status":"online","version":"13.1.0-zenith"}` |
| Puertos alias (3000/3030/8090/8080) | escuchando en 127.0.0.1 |
| Túnel `/health` | `200 {"status":"ok","version":"13.3.0"}` |

El puente (`deploy/web_bridge.py`) es **temporal**: en cuanto se despliegue el
binario con alias de puertos y `/` exento del gate de API key, se puede borrar.

### CONTRATO DE VOLCADO (fijado con el bus de coordinación)

- Ruta de entrada en el móvil: `/data/local/tmp/megadaemon/incoming/<agent_id>/`
- Manifiesto: `/data/local/tmp/megadaemon/incoming/<agent_id>/MANIFEST.ndjson`
  (una línea JSON por artefacto):
  `{"id","name","path","origin","type":"apk|dex|so|json|log|txt|binary","size_bytes","sha256","summary","ts"}`
- Informe: `.../<agent_id>/REPORT.md`
- Binarios > 1 MB: `POST /api/chat_upload` con `{"filename","data_b64"}`, o `fs_write`
  con `base64: true` en trozos.
- Copia en el estado compartido: `sync/externo/<agent_id>/<dataset>.ndjson` + `README.md`
  como índice (una línea por hallazgo: `source`, `ts`, `confidence`, `evidence`).
- **El módulo `apk_inventory` acepta CUALQUIER ruta absoluta del dispositivo**
  (`analyze_apk(path)`), no sólo APKs instaladas en `/data/app`: los APK que
  traiga un agente externo en `incoming/` se analizan igual (entradas, `classes*.dex`,
  `.so` nativas, assets, `resources.arsc` y manifiesto AXML con permisos, componentes
  y huella) y se verifican contra el `sha256` del manifiesto.

