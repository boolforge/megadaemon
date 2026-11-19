# MegaDaemon v13.3 — Harness nativo + Motor de sincronización de conversaciones

> Fecha: **2026-11-19**. Todo lo que se afirma aquí está **medido en el
> dispositivo real** (Galaxy S21 Ultra, Exynos 2100, Android 14), no estimado.

## 1. Qué se ha hecho en esta iteración

| Antes (v13.2) | Ahora (v13.3) |
|---|---|
| El agente se conectaba al móvil con un **servidor MCP en Python** (`android-root`), que además era *la chapucilla* que había hecho el usuario | **megadaemon ES el harness**: expone un **MCP nativo en Rust** por HTTP con el mismo catálogo de herramientas, y además lo he usado yo mismo para todo el trabajo de este informe |
| Control del dispositivo = *shell* libre con `su -mm -c` | **11 herramientas tipadas** con validación de parámetros, timeouts, topes de salida, auditoría y política destructiva |
| Historial de las apps de IA: inaccesible | **152 conversaciones / 1.566 mensajes / 15,1 MB** extraídos, normalizados, indexados y **buscables** desde el MCP |
| Demonio sin supervisor fiable | Supervisor Magisk **v2**: reinicia cockpit + túnel + MCP Python y **sincroniza cada 30 min** |

## 2. El harness: megadaemon como servidor MCP

### 2.1 Transporte

El cockpit (puerto 9090) expone ahora **`POST /mcp`** con JSON-RPC 2.0
(Streamable HTTP). El **mismo catálogo** que el transporte stdio: una sola
implementación (`mcp_server::handle_rpc`) para los dos transportes.

```
POST https://<túnel>.trycloudflare.com/mcp
Headers: X-API-Key: <api key>            # control de acceso remoto
         X-Seere-Auth-Token: <token>      # autorización de escritura
         Content-Type: application/json
```

`initialize` → `tools/list` → `tools/call`. Lote (batch) soportado.

### 2.2 Catálogo (26 herramientas)

| Grupo | Herramientas |
|---|---|
| RE (v13.0) | `minimax_chat`, `swarm_chat`, `megadaemon_die_binwalk`, `megadaemon_rizin`, `megadaemon_cirx_analyze`, `megadaemon_mba_simplify`, `megadaemon_npu_cv` |
| **Dispositivo (nuevo)** | `system_overview`, `root_exec`, `fs_read`, `fs_write`, `find_duplicates`, `props_and_settings`, `selinux_control`, `magisk_module`, `kernel_and_blocks`, `app_control`, `screen_and_ui` |
| **Sync (nuevo)** | `sync_conversations`, `search_conversations`, `conversation_stats`, `list_conversations`, `get_conversation`, `sync_web_conversations`, `harvest_sessions`, `ingest_export` |

### 2.3 Por qué es mejor que el MCP en Python

| Python (la chapucilla) | Rust (megadaemon v13.3) |
|---|---|
| `subprocess.run("su -mm -c " + cmd)` | `su -mm -c` con **parámetros tipados**, nunca texto libre del LLM |
| Sin validación: el modelo podía pasar cualquier shell | Enums en el esquema, validación de paquetes/rutas/permisos y **escape `sh_quote`** de todo valor interpolado |
| Sin timeout (podía colgarse) | Timeout obligatorio (30 s; máx. 300 s) con *kill* del hijo |
| Sin tope de salida (podía reventar la memoria) | Tope por defecto 64 KB, máximo 512 KB, con marca de truncado |
| `rm -rf` sin fricción | **Política destructiva**: `rm -rf /`, `mkfs`, `dd of=/dev/block`… exigen `confirm_destructive=true` |
| Sin registro | **Auditoría** en `audit.ndjson` (operación, objetivo, resultado) |
| Duplicados con `jdupes` (hay que compilarlo a mano) | **SHA-256 nativo en streaming**, sin dependencias externas |
| Errores en texto plano | Errores **estructurados** (`ok`, `error`, `hint`) + `structuredContent` MCP |
| Una sola clave | **Dos capas**: API key (remoto) + token de sesión (escritura) |

## 3. Motor de sincronización de conversaciones

### 3.1 Arquitectura

```
apps del dispositivo                    megadaemon (Rust)
─────────────────────                   ─────────────────────
SQLite privado ──┐                      ┌─────────────────────┐
WebView/LevelDB ──┼── extractor ──────► │ normalización       │
cookies en claro ─┘   (Python,        │ almacén NDJSON      │
                      embebido en       │ índice + búsqueda   │
                      el binario)      │ MCP + API + CLI     │
                                        └─────────────────────┘
```

El extractor Python va **embebido en el binario** (`include_str!`) y se
escribe en el directorio de estado en tiempo de ejecución: no hay ficheros
sueltos que instalar. Se ejecuta con el `python3` de Termux porque los
esquemas de las apps cambian y SQLite/JSON se analizan mucho mejor ahí; **toda
la orquestación, el almacén, el índice y la búsqueda están en Rust**.

### 3.2 Esquemas descubiertos (auditoría real del dispositivo)

| App | Paquete | Almacén | Contenido | Resultado |
|---|---|---|---|---|
| Claude | `com.anthropic.claude` | `databases/acc_*_conversation_snapshots.db` | `snapshot_headers` (22) + `snapshot_messages` (424, campo `body` JSON con bloques `text`/`thinking`/`tool_use`/`tool_result`) | **22 conversaciones / 423 mensajes** |
| DeepSeek | `com.deepseek.chat` | `databases/deepseek_chat_<uuid>.db` | `chat_session_list` (101) + una tabla `chat_session_messages_<id>` por sesión (`fragments` JSON: REQUEST/THINK/SEARCH/TOOL_SEARCH) | **101 / 1.022** |
| Kimi | `com.moonshot.kimichat` | `databases/kimi_*_chat.db` | `DBChatItem` + `DBChatMessage` (`extra` JSON con `blocks[].text/think`) | **24 / 70** |
| ChatGPT | `com.openai.chatgpt` | `files/sessions/guest-*/default/databases/*_conversations.db` | `DBConversation` + `DBMessage` + `DBMessageChunk` (`chunk` JSON con `content.content`) | **5 / 51** |
| Gemini | `com.google.android.apps.bard` | — | **no guarda nada en local** (sólo 523 KB de prefs) | histórico **sólo servidor** |
| Perplexity / Mistral / Copilot / Poe | — | sin tablas de conversación | — | 0 |

### 3.3 Lo que falta para el 100 %: el historial en la nube

El historial completo de las cuentas reales vive en el servidor. Se
implementaron **tres vías**, todas en megadaemon:

1. **Cosecha de sesión** (`harvest_sessions`): lee las cookies en claro de las
   WebView de las apps, el token de la app de DeepSeek (`app_user_info.token`)
   y la `last_app_key` de Kimi. Resultado medido: 4 sesiones cosechadas.
2. **Pull por API** (`sync_web_conversations`): usa esas sesiones contra los
   endpoints reales (`chatgpt.com/backend-api/conversations`,
   `chat.deepseek.com/api/v0`, `kimi.com/api/chat`, `claude.ai/api`,
   `gemini.google.com`). El pipeline **funciona** (ChatGPT responde 200), pero:
   * la sesión de la app de ChatGPT en este dispositivo es de **invitado**
     (`/backend-api/me` → `email: ""`, `object: ""`) → 0 conversaciones;
   * Kimi → `401` (app key caducada);
   * DeepSeek → HTML anti-bot (Cloudflare) → necesita sesión de navegador real.
3. **Ingesta de exportación oficial** (`ingest_export`): parsea
   `conversations.json` de OpenAI (formato `mapping`), de Anthropic
   (`chat_messages`) y de Google Takeout. **Probado con round-trip real**: se
   exportó desde el almacén, se reingirió y quedó indexado y buscable.

**El bloqueo real está identificado y es de una sola cosa**: las cookies de
Chrome (donde están las cuentas reales) están cifradas con una clave del
AndroidKeyStore **no exportable** — ni root puede leerla de disco. La
solución es inyectarse en el proceso, y para eso está el script
`rust-core/assets/frida_chrome_cookies.js`:

```bash
# en el móvil (Termux), con frida-server en marcha
frida -U -n com.android.chrome -l frida_chrome_cookies.js --no-pause
```

El script engancha OkHttp, `URL.openConnection()` y el barrido de Local
Storage, y emite NDJSON con las cabeceras `Cookie`/`Authorization` en claro,
listas para que `harvest_sessions` las incorpore.

### 3.4 Automatización

El supervisor (`/data/adb/service.d/99-megadaemon.sh`) ahora:

* mantiene vivos **cockpit + túnel Cloudflare + MCP Python** (con backoff);
* **sincroniza cada 30 minutos** (`megadaemon-core sync`), idempotente;
* rota el log (10 MB × 5) y guarda la URL del túnel en `tunnel.url`.

## 4. API y CLI

```
POST /mcp                      JSON-RPC 2.0 (MCP nativo, 26 herramientas)
GET  /health  /metrics         salud y métricas (sin token de sesión)
POST /api/sync/run             sincronización local
POST /api/sync/harvest         cosecha de sesiones
POST /api/sync/web             pull desde la nube
POST /api/sync/ingest          ingesta de exportación oficial
POST /api/sync/status          estado del almacén
POST /api/conversations/search búsqueda full-text cross-servicio
POST /api/conversations/list   listado

megadaemon-core sync            sincronización (para cron)
megadaemon-core conv-search Q   búsqueda
megadaemon-core conv-stats      estado
```

## 5. Despliegue en el dispositivo

| Dato | Valor |
|---|---|
| Binario | ARM64 cross-compilado con NDK r26c, **3.19 MB**, `SHA-256` verificado byte a byte tras la subida |
| Ruta | `/data/local/tmp/megadaemon/megadaemon-core` (755) |
| Estado | `/data/local/tmp/megadaemon/state/` (API key, token, almacén de conversaciones) |
| Supervisor | `/data/adb/service.d/99-megadaemon.sh` (755) |
| Extractor | `/data/local/tmp/megadaemon/state/sync_extract.py` (escrito por el propio binario) |
| Sincronización | `/data/local/tmp/megadaemon/state/sync/<servicio>.ndjson` |

### Rollback

```bash
cp /data/local/tmp/megadaemon/megadaemon-core.v13.2.bak \
   /data/local/tmp/megadaemon/megadaemon-core
```

## 6. Tests

`cargo test --release`: **129 tests** verdes, de los cuales 13 nuevos:

* `android`: escape de shell, detección de comandos destructivos, catálogo
  completo, enums en los esquemas, rechazo de rutas inválidas, confirmación
  exigida en `rm -rf /`.
* `sync`: tokenización, clave de almacén, servicios válidos, extractor
  embebido, búsqueda vacía.
* `sync_web`: base64 propio, detección de formato de exportación, parseo de
  exportaciones de OpenAI y Anthropic, política de cookies.
