# Sync de chats por servicio — dónde están los datos y hasta dónde se llega

Módulo: `rust-core/src/sync_chats.rs` · CLI: `rust-core/src/bin/md-sync-chats.rs`

Este documento es la **fuente de verdad operativa** del sync de chats.  Para
cada servicio se dice: de dónde salen los datos, con qué **esquema real
medido**, por qué vía se acceden, hasta qué **nivel** se llega y **qué falta**
para llegar al completo.

> Principio del módulo: **nada se inventa**.  Si el almacén está cifrado, el
> estado es `cifrado_no_descifrado` y la lista de mensajes queda **vacía**; si
> no hay dispositivo, el estado es `sin_acceso`.  Nunca se fabrica un
> "mensaje" para rellenar un hueco.

---

## 1. El modelo unificado

```jsonc
{
  "service": "telegram",
  "conversation_id": "chat_42",
  "title": "Ana",
  "messages": [
    { "id": "3", "ts": 1700000120, "author": "peer:9", "text": "¿que tal?", "kind": "texto", "verified": true }
  ],
  "origin": "local_db",        // local_db | web_api | export | notificaciones
  "access": "completo",        // completo | metadatos | cifrado_no_descifrado
  "updated_at": 1700000120,
  "source": "/data/data/org.telegram.messenger/databases/msgstore.db",
  "note": ""
}
```

* `origin` = **de dónde** salió el dato.
* `access` = **cuánto** se ha podido leer:
  * `completo`: todos los mensajes.
  * `metadatos`: sólo la lista (contactos, chats, conversaciones, número de
    no leídos) o el texto que el sistema ya mostraba en una notificación.
  * `cifrado_no_descifrado`: el almacén está cifrado (SQLCipher) y **no** se
    ha descifrado.
* `verified: false` marca las entradas que **no** son mensajes (p. ej. un
  contador de no leídos leído de la tabla de chats).  La web y el MCP deben
  poder distinguirlas: `verified_messages()` cuenta sólo las verificadas.

**Clave de merge (idempotencia):** `servicio::conversation_id::message_id`.
El merge es a nivel de **mensaje**, no de conversación: re-sincronizar no
duplica nada y un mensaje ya comido no se vuelve a traer.

**Incremental:** cada servicio tiene su cursor (`<state_dir>/sync_chats/cursors.json`)
con `last_ts` + `last_id`.  En Telegram el cursor se empuja **al SQL**
(`WHERE date > <last_ts>`); en Discord, el `after=<snowflake>` va en la URL;
en el resto se filtra en el cliente.

---

## 2. Tabla por servicio

| Servicio | Dónde están los datos | Esquema real medido | Vía de acceso | Nivel | Qué falta |
|---|---|---|---|---|---|
| **WhatsApp** (`whatsapp`) | `/data/data/com.whatsapp/databases/msgstore.db` (+ `.crypt15`) y `wa.db` | `msgstore.db`: SQLCipher (cabecera **no**-SQLite) → ilegible. `wa.db`: tabla de contactos (`jid`, `display_name`) y de chats (`jid`, `unread_count`, `message_count`, `last_message_time`) | root (`su -mm -c`) + `dumpsys notification --noredact` | `metadatos` | El **texto** de los mensajes: la clave AES vive en el AndroidKeyStore y no es exportable. Hace falta hook in-process (Frida) o la exportación oficial. **No se intenta descifrar.** Lo que sí es real y verificable: la **tabla de contactos** (`wa_contacts`: jid + nombre) y el **texto de las notificaciones** (origen `notificaciones`) |
| **WhatsApp Business** (`whatsapp_business`) | `/data/data/com.whatsapp.w4b/databases/{msgstore.db,wa.db}` | igual que WhatsApp (mismo motor, otro paquete) | root + `dumpsys notification --noredact` | `metadatos` | mismo límite de cifrado |
| **Telegram** (`telegram`) | `/data/data/org.telegram.messenger/databases/*.db` | SQLite **sin cifrar**: `messages(uid, chat_id, from_id, date, message)`, `chats/dialogs`, `users`, `info` | root: se copia la BD (con `-wal`/`-shm`) a un snapshot y se lee con SQLite | `completo` | Nada para el texto; sólo cambia el formato de la columna `message` entre versiones (se detecta por columnas) |
| **Signal** (`signal`) | `/data/data/org.thoughtcrime.securesms/app_database/*.db` (SQLCipher) + notificaciones | `signal.db` cifrado con SQLCipher (cabecera no-SQLite): la lista de tablas **no** es legible. Sí lo es la capa de notificaciones (`pkg`, `android.title`, `android.text`) | root + `dumpsys notification --noredact` | `cifrado_no_descifrado` (metadatos por notificaciones) | **Todo** el contenido de los mensajes. **No** se intenta romper el cifrado de Signal: la clave está atada al dispositivo |
| **Discord** (`discord`) | API oficial con el token del usuario (`discord.com/api/v9`) o la exportación oficial (`messages.json`) | API: `{channels:[{id,name,type,guild:{name}}]}` y `{messages:[{id,timestamp,content,author:{name,username}}]}`. Exportación: `{"channels":[{"id","name","messages":[...]}]}` (mismo formato de mensaje) | `curl` del dispositivo con cabecera `Authorization` (token en fichero `0600`) o ingesta del `messages.json` | `completo` | El token: hay que pegarlo una vez (nunca se guarda en el repo). Sin token, sólo queda la exportación oficial |
| **Grok (xAI)** (`grok`) | Web de Grok: sesión del navegador/app (cookies) o exportación oficial | **NO VERIFICADO**: los endpoints internos de `grok.com` hay que medirlos en el móvil (ver §3). La API pública `https://api.x.ai/` **no** da el historial de chat | `curl` del dispositivo con las cookies de la sesión (hook de Frida `assets/frida_chrome_cookies.js` o la exportación oficial) | `completo` (si hay sesión) | El paquete Android verificado es `ai.x.grok` (id previo: `com.x.ai`); el historial local de la app no está mapeado |
| **ChatGPT** (`chatgpt`) | `/data/data/com.openai.chatgpt/files/sessions/*/default/databases/*_conversations.db` | `DBMessageChunk.chunk.content.content` (medido) | `crate::sync::run_sync` (extractor Python embebido) | `completo` | nada |
| **Claude** (`claude`) | `/data/data/com.anthropic.claude/databases/*conversation_snapshots*.db` | `snapshot_messages.body` (JSON con `blocks[].content.block.text` / `.thinking`) (medido) | `crate::sync::run_sync` | `completo` | nada |
| **Gemini** (`gemini`) | servidor (la app Android **no** guarda historial local) | no hay store local: la app no persiste el chat. Sólo la web (`gemini.google.com`) y la exportación de Takeout | `crate::sync_web::web_pull` (necesita la cookie `__Secure-1PSID` de la cuenta real) | `metadatos` | La cookie de la cuenta real (la sesión cosechada es de invitado) |
| **DeepSeek** (`deepseek`) | `/data/data/com.deepseek.chat/databases/deepseek_chat_*.db` | `chat_session_list` + `chat_session_messages_<id>` (medido) | `crate::sync::run_sync` | `completo` | nada |
| **Kimi** (`kimi`) | `/data/data/com.moonshot.kimichat/databases/kimi_*_chat.db` | `DBChatMessage.extra.blocks[].text` / `.think` (medido) | `crate::sync::run_sync` | `completo` | nada |
| **Mistral** (`mistral`) | `/data/data/ai.mistral.chat/databases/*.db` | genérico (el extractor busca tablas con estructura de conversación); esquema exacto **NO VERIFICADO** | `crate::sync::run_sync` | `completo` | el esquema exacto de la app |
| **Perplexity** (`perplexity`) | `/data/data/ai.perplexity.app.android/databases/*.db` | genérico; esquema exacto **NO VERIFICADO** | `crate::sync::run_sync` | `completo` | el esquema exacto de la app |
| **Copilot** (`copilot`) | `/data/data/com.microsoft.copilot/databases/*.db` | genérico; esquema exacto **NO VERIFICADO** | `crate::sync::run_sync` | `completo` | el esquema exacto de la app |
| **Poe** (`poe`) | `/data/data/com.poe.android/databases/*.db` | genérico; esquema exacto **NO VERIFICADO** | `crate::sync::run_sync` | `completo` | el esquema exacto de la app |

Los servicios de IA **no se duplican**: `sync_chats` llama a `crate::sync`
(ruta local) y a `crate::sync_web` (ruta web) y convierte su salida al modelo
unificado.  **Grok** es el único añadido, con su propio conector web (mismo
patrón que `sync_web.rs`: `curl` del dispositivo + cookies).

---

## 3. Comandos EXACTOS de medición en el móvil

Cada bloque se ejecuta en el móvil (con `su`, o desde Termux con root).

### WhatsApp

```bash
su -mm -c 'ls -l /data/data/com.whatsapp/databases/'
su -mm -c 'head -c 16 /data/data/com.whatsapp/databases/msgstore.db | xxd'
su -mm -c 'head -c 16 /data/data/com.whatsapp/databases/wa.db | xxd'
su -mm -c 'dumpsys notification --noredact | grep -A3 com.whatsapp'
```

* Lo que se espera: `msgstore.db` **no** empieza por `SQLite format 3` (SQLCipher)
  y `wa.db` **sí** (metadatos en claro).

### WhatsApp Business

```bash
su -mm -c 'ls -l /data/data/com.whatsapp.w4b/databases/'
su -mm -c 'dumpsys notification --noredact | grep -A3 com.whatsapp.w4b'
```

### Telegram

```bash
su -mm -c 'ls -l /data/data/org.telegram.messenger/databases/'
su -mm -c 'cp /data/data/org.telegram.messenger/databases/*.db /data/local/tmp/'
sqlite3 /data/local/tmp/msgstore.db '.tables'
sqlite3 /data/local/tmp/msgstore.db 'PRAGMA table_info(messages)'
sqlite3 /data/local/tmp/msgstore.db 'PRAGMA table_info(chats)'
```

* Lo que se espera: cabecera SQLite **en claro** y tablas `messages`, `chats`,
  `users`, `info`.

### Signal

```bash
su -mm -c 'ls -l /data/data/org.thoughtcrime.securesms/app_database/'
su -mm -c 'head -c 16 /data/data/org.thoughtcrime.securesms/app_database/signal.db | xxd'
su -mm -c 'dumpsys notification --noredact | grep -A3 org.thoughtcrime.securesms'
```

* Lo que se espera: `signal.db` **no** empieza por `SQLite format 3` ⇒
  `cifrado_no_descifrado`.

### Discord

```bash
su -mm -c 'ls -l /data/data/com.discord/app_localstorage/leveldb'
su -mm -c 'curl -s -H "Authorization: <TOKEN>" https://discord.com/api/v9/users/@me/channels | head -c 400'
su -mm -c 'find /sdcard -name messages.json 2>/dev/null'
```

* Lo que se espera: el `leveldb` **no** es una base de datos utilizable ⇒ la
  vía real es la API con el token o la exportación oficial.

### Grok (xAI)

```bash
su -mm -c 'pm list packages | grep -i grok'
su -mm -c 'ls -l /data/data/ai.x.grok/app_webview/Default/'
su -mm -c 'dumpsys activity activities | grep -i grok'
su -mm -c 'curl -s -o /dev/null -w "%{http_code}" https://grok.com/'
```

* Lo que se espera: el paquete `ai.x.grok` instalado y el `WebView/Default/Cookies`
  (con las cookies **en claro**; las cifradas por el keystore necesitan el hook
  de Frida `assets/frida_chrome_cookies.js`).
* Los endpoints REST internos de `grok.com` que usa el módulo
  (`GROK_ENDPOINTS` en `sync_chats.rs`) están marcados como **NO VERIFICADO**:
  hay que medirlos con el token/cookie reales y ajustarlos si responden 404.

### Apps de IA (medición del esquema)

```bash
su -mm -c 'ls /data/data/com.openai.chatgpt/files/sessions/*/default/databases/'
su -mm -c 'ls /data/data/com.anthropic.claude/databases/'
su -mm -c 'ls /data/data/com.deepseek.chat/databases/'
su -mm -c 'ls /data/data/com.moonshot.kimichat/databases/'
su -mm -c 'ls /data/data/ai.mistral.chat/databases/'
su -mm -c 'ls /data/data/ai.perplexity.app.android/databases/'
su -mm -c 'ls /data/data/com.microsoft.copilot/databases/'
su -mm -c 'ls /data/data/com.poe.android/databases/'
su -mm -c 'ls /data/data/com.google.android.apps.bard/databases/'
```

---

## 4. Los límites, con honestidad

| Límite | Por qué | Qué hace el módulo |
|---|---|---|
| WhatsApp cifrado | La clave AES del almacén vive en el AndroidKeyStore del proceso de WhatsApp; no es exportable **ni con root** | Estado `cifrado_no_descifrado` para `msgstore.db`; se usan `wa.db` (metadatos) y las notificaciones (texto ya visible) |
| Signal cifrado | Todo el contenido va en SQLCipher con clave atada al dispositivo | Estado `cifrado_no_descifrado`; sólo la lista de conversaciones por notificaciones. **No** se intenta descifrar |
| Cookies de Chrome | Cifradas por el AndroidKeyStore: no exportables | Para las webs hay que usar el hook de Frida (`assets/frida_chrome_cookies.js`) o la exportación oficial del servicio |
| Cloudflare | Rechaza la huella TLS de Rust (401/403 con cookie válida) | Las peticiones web van con el `curl` **del dispositivo** (mismo patrón que `sync_web.rs`) |
| Gemini | La app Android **no** persiste el historial | Nivel `metadatos`: sólo web (con la cookie de la cuenta real) o Takeout |
| Discord sin token | La API exige el token del usuario | `sin_acceso` con el motivo; el token va a `<state_dir>/sync_chats/discord_token` (`0600`) |

---

## 5. Cómo se usa

```bash
# catálogo: de dónde están los datos y qué falta
md-sync-chats list

# pull de un servicio (idempotente e incremental)
md-sync-chats pull --servicio telegram
md-sync-chats pull --servicio discord --json

# estado del almacén y de los cursores
md-sync-chats status

# token de Discord (0600, fuera del repo)
md-sync-chats token --set "<token>"
md-sync-chats token --show

# exportación oficial (fichero local o del dispositivo)
md-sync-chats ingest /sdcard/Download/messages.json

# trae al modelo unificado lo que ya sincronizó el motor de IA (crate::sync)
md-sync-chats migrate
```

Sin móvil (sandbox), `pull` responde con el estado real:

```json
{ "ok": false, "status": "sin_acceso", "access": "metadatos",
  "errors": ["no se encuentra la base de datos de Telegram en el dispositivo (o no hay root): sin acceso"] }
```

---

## 6. Enganche con la app (MCP + web)

El módulo expone `tool_definitions()` y `dispatch()` con el mismo contrato que
el resto del repo, para que la web y el MCP vean siempre `origin` y `access`:

* `sync_chats_pull` — sincroniza un servicio (con su cursor).
* `sync_chats_status` — estado del almacén y de los cursores.
* `sync_chats_list` — conversaciones con origen, nivel y nº de mensajes verificados.
* `sync_chats_catalog` — el catálogo de la tabla de arriba.
* `sync_chats_conversation` — una conversación completa.
* `sync_chats_ingest_export` — ingesta de una exportación oficial.

**Para que compile hace falta una línea en `rust-core/src/lib.rs`** (el módulo
no lo añade él para no pisar el `lib.rs` del repo):

```rust
pub mod sync_chats;
```

y, en `rust-core/src/mcp_server.rs`, enganchar las tools:

```rust
if let Some(chats_arr) = crate::sync_chats::tool_definitions().as_array().cloned() {
    arr.extend(chats_arr);
}
// …y en el dispatch:
if let Some(r) = crate::sync_chats::dispatch(name, args) {
    return Some(r);
}
```

Sin ese enganche, el CLI (`md-sync-chats`) funciona igual: es la vía directa
desde la app.

---

## 7. Tests

Todos deterministas, **sin red y sin móvil**:

| Test | Qué comprueba |
|---|---|
| `los_nombres_de_origen_y_nivel_son_los_establecidos` | Los cuatro orígenes y los tres niveles son los del contrato |
| `la_clave_de_mensaje_es_servicio_conversacion_mensaje` | La clave de merge es de tres partes |
| `el_nivel_de_acceso_sube_pero_no_baja_al_fusionar` | Un pull parcial no degrada una conversación completa |
| `el_merge_es_idempotente_por_mensaje` | Mergear dos veces lo mismo **no** duplica mensajes |
| `el_cursor_solo_deja_pasar_lo_nuevo` | El filtro incremental (ts + id) |
| `el_cursor_avanza_con_el_mensaje_mas_reciente` | El cursor avanza con (ts, id) |
| `telegram_lee_el_fixture_y_mapea_chat_y_mensajes` | Fixture SQLite real: chat↔mensajes, autor propio vs peer, título |
| `telegram_es_incremental_con_el_cursor` | Segunda pasada con cursor: 0 mensajes nuevos |
| `una_bd_cifrada_no_devuelve_mensajes_inventados` | Cabecera no-SQLite ⇒ `cifrado_no_descifrado` y **0** mensajes |
| `whatsapp_detecta_el_almacen_cifrado_y_no_inventa_mensajes` | Idem para WhatsApp |
| `whatsapp_lee_los_contactos_reales_y_no_inventa_mensajes` | Fixture `wa.db`: contactos reales (jid + nombre) y chats como metadatos con `verified: false` |
| `las_notificaciones_son_texto_real_y_de_origen_notificaciones` | Fixture de `dumpsys`: origen `notificaciones`, nivel `metadatos`, texto literal |
| `las_notificaciones_respetan_el_cursor` | Las notificaciones también son incrementales |
| `signal_esta_cifrado_y_no_se_intenta_descifrar` | Signal: `cifrado_no_descifrado`, sin mensajes, sin tocar el cifrado |
| `la_exportacion_de_discord_se_normaliza` | Fixture `messages.json` ⇒ modelo unificado (ts ISO, autor, origen `export`) |
| `la_exportacion_de_discord_respeta_el_cursor` | Incremental en la exportación |
| `el_token_de_discord_se_guarda_en_un_fichero_0600` | Permisos `0600` del fichero de credenciales |
| `la_url_de_mensajes_de_discord_lleva_el_cursor` | `after=<snowflake>` en la URL |
| `grok_parsea_una_exportacion_y_avisa_de_los_endpoints_no_verificados` | Parser de Grok + marca `NO VERIFICADO` en el catálogo |
| `el_catalogo_cubre_los_cuatro_chats_y_grok` | No falta ningún servicio |
| `whatsapp_y_signal_declaran_el_limite_de_cifrado` | El catálogo documenta el límite |
| `cada_servicio_tiene_sus_ordenes_de_medicion` | Todo servicio trae sus comandos de medición |
| `la_doc_generada_tiene_una_fila_por_servicio` | La doc sale del catálogo (no se separan) |
| `una_conversacion_legacy_se_convierte_al_modelo_unificado` | Conversión desde `sync.rs` |
| `el_iso8601_de_discord_se_convierte_bien` | Marcas de tiempo sin dependencia de fecha |
| `el_base64_propio_descodifica_bytes_binarios` | Base64 → bytes (BDs binarias) |
