//! MegaDaemon v13.5 — Motor de sincronización de **CHATS** (WhatsApp,
//! Telegram, Signal, Discord) y ampliación del catálogo de servicios SaaS
//! agénticos (Grok).
//!
//! Qué aporta sobre [`crate::sync`] (que ya sincroniza ChatGPT, Claude,
//! Gemini, DeepSeek, Kimi, Perplexity, Mistral, Copilot y Poe):
//!
//! 1. **Adaptadores por servicio**, cada uno con su **origen de datos real**,
//!    su **esquema medido** y su **nivel de acceso** declarado.  No hay un
//!    extractor genérico que "adivine": cada servicio dice de dónde saca los
//!    datos y hasta dónde puede llegar.
//! 2. **Modelo unificado** de conversación/mensaje, con `origen`
//!    (`local_db` | `web_api` | `export` | `notificaciones`) y **nivel de
//!    acceso** (`completo` | `metadatos` | `cifrado_no_descifrado`).  La web
//!    y el MCP pueden distinguir un mensaje REAL de una notificación o de un
//!    hueco cifrado: nunca se muestra un "mensaje" que no existe.
//! 3. **Merge idempotente** por `servicio::conversation_id::message_id` (a
//!    nivel de **mensaje**, no de conversación) e **incremental**: cada
//!    servicio guarda su cursor y sólo se descarga lo nuevo.
//! 4. **Sin inventar datos**: si WhatsApp o Signal están cifrados con
//!    SQLCipher, el estado es `cifrado_no_descifrado` y la lista de mensajes
//!    queda VACÍA.  No se intenta romper el cifrado de Signal ni de WhatsApp.
//!
//! Relación con el resto del repo:
//!
//! * `crate::sync` → motor de sync de las apps de IA (local + web).  Este
//!   módulo **lo consume** (no lo duplica): para los servicios de IA llama a
//!   `sync::run_sync` / `sync_web::web_pull` y convierte su salida al modelo
//!   unificado.  Grok es el único servicio de IA **nuevo** y por eso lleva
//!   su propio conector web (mismo patrón que `sync_web.rs`: `curl` del
//!   dispositivo + cookies).
//! * `crate::messaging` → ya sabe leer Telegram y WhatsApp.  Este módulo
//!   **reutiliza** su detección de esquema, su decodificador de blobs y su
//!   parser de notificaciones, y le añade el cursor incremental y el modelo
//!   unificado.
//! * `crate::sync_web` → `curl` del dispositivo (Cloudflare rechaza la
//!   huella TLS de Rust) y decodificador base64.  También se reutiliza.
//!
//! Reglas del arnés:
//!
//! * Rust puro, identificadores en inglés, comentarios en español.
//! * Sin red en los tests: todos los fixtures son locales (SQLite temporal,
//!    JSON y volcados de `dumpsys`).
//! * Ningún secreto en el repo: el token de Discord vive en un fichero `0600`
//!   dentro del `state_dir`.

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

// ========================================================================= //
// 1. Modelo unificado
// ========================================================================= //

/// De dónde procede el dato.
///
/// - `local_db`: base de datos privada de la app en el dispositivo.
/// - `web_api`: API del servicio (token/cookie de la sesión del usuario).
/// - `export`: exportación oficial del servicio.
/// - `notificaciones`: capa de notificaciones del sistema (lo único que se
///   puede leer sin descifrar nada).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    #[serde(rename = "local_db")]
    LocalDb,
    #[serde(rename = "web_api")]
    WebApi,
    #[serde(rename = "export")]
    Export,
    #[serde(rename = "notificaciones")]
    Notifications,
}

impl Default for Origin {
    fn default() -> Self {
        Origin::LocalDb
    }
}

impl Origin {
    /// Nombre estable (el mismo en el JSON y en la documentación).
    pub fn as_str(&self) -> &'static str {
        match self {
            Origin::LocalDb => "local_db",
            Origin::WebApi => "web_api",
            Origin::Export => "export",
            Origin::Notifications => "notificaciones",
        }
    }

    /// Parsea el nombre estable (`None` si no es ninguno de los cuatro).
    pub fn parse(s: &str) -> Option<Origin> {
        match s {
            "local_db" => Some(Origin::LocalDb),
            "web_api" => Some(Origin::WebApi),
            "export" => Some(Origin::Export),
            "notificaciones" => Some(Origin::Notifications),
            _ => None,
        }
    }
}

/// Cuánto del contenido real se ha podido leer.
///
/// El orden importa (`Ord`): `max()` se usa al fusionar, de modo que una
/// conversación que pasa de `metadatos` a `completo` **sube** de nivel y una
/// que ya era `completo` no baja por un pull parcial.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AccessLevel {
    /// El almacén está cifrado y no se ha descifrado (WhatsApp, Signal).
    #[serde(rename = "cifrado_no_descifrado")]
    CifradoNoDescifrado,
    /// Sólo se han podido leer metadatos (contactos, chats, lista de
    /// conversaciones, notificaciones).
    #[serde(rename = "metadatos")]
    Metadatos,
    /// Conversación completa (todos los mensajes).
    #[serde(rename = "completo")]
    Completo,
}

impl Default for AccessLevel {
    fn default() -> Self {
        AccessLevel::Metadatos
    }
}

impl AccessLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccessLevel::CifradoNoDescifrado => "cifrado_no_descifrado",
            AccessLevel::Metadatos => "metadatos",
            AccessLevel::Completo => "completo",
        }
    }

    pub fn parse(s: &str) -> Option<AccessLevel> {
        match s {
            "cifrado_no_descifrado" => Some(AccessLevel::CifradoNoDescifrado),
            "metadatos" => Some(AccessLevel::Metadatos),
            "completo" => Some(AccessLevel::Completo),
            _ => None,
        }
    }
}

/// Un mensaje dentro de una conversación.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Identificador estable del mensaje **dentro de la conversación**.
    ///
    /// Es la tercera parte de la clave de merge
    /// (`servicio::conversation_id::message_id`).  Si el esquema de origen no
    /// da un id, se sintetiza uno determinista a partir de (ts, autor, texto)
    /// para que la re-sincronización no duplique nada.
    #[serde(default)]
    pub id: String,
    /// Segundos desde epoch (UTC).  `0` si el origen no lo da.
    #[serde(default)]
    pub ts: i64,
    /// Quién escribió: `"user"`, `"peer"` o el nombre real si se conoce.
    #[serde(default)]
    pub author: String,
    /// Texto literal (nunca inventado).
    #[serde(default)]
    pub text: String,
    /// Tipo de entrada: `texto`, `notificacion`, `sistema`, `adjunto`,
    /// `imagen`.
    #[serde(default)]
    pub kind: String,
    /// `false` = **NO VERIFICADO**: el contenido no procede del esquema real
    /// del servicio (p. ej. un placeholder mientras no se conoce el
    /// esquema).  Un mensaje no verificado NUNCA se cuenta como mensaje.
    #[serde(default = "verified_default")]
    pub verified: bool,
}

fn verified_default() -> bool {
    true
}

/// Una conversación normalizada (el mismo esquema para todos los servicios).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conversation {
    /// Servicio (`whatsapp`, `telegram`, `signal`, `discord`, `grok`, ...).
    pub service: String,
    /// Identificador de la conversación **dentro del servicio**.
    pub conversation_id: String,
    /// Título legible (nombre del chat, del canal, ...).
    #[serde(default)]
    pub title: String,
    /// Mensajes (ordenados por `(ts, id)`).
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    /// De dónde han salido los datos.
    pub origin: Origin,
    /// Cuánto se ha podido leer.
    pub access: AccessLevel,
    /// Última actividad conocida (epoch).
    #[serde(default)]
    pub updated_at: i64,
    /// Ruta exacta o endpoint del que salió todo.
    #[serde(default)]
    pub source: String,
    /// Explicación (por qué el acceso no es completo, qué falta, ...).
    #[serde(default)]
    pub note: String,
}

impl Conversation {
    /// Clave de almacenamiento de la conversación.
    pub fn key(&self) -> String {
        conversation_key(&self.service, &self.conversation_id)
    }

    /// Número de mensajes **verificados** (los no verificados no cuentan).
    pub fn verified_messages(&self) -> usize {
        self.messages.iter().filter(|m| m.verified).count()
    }
}

/// Cursor incremental de un servicio.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cursor {
    /// Servicio al que pertenece.
    #[serde(default)]
    pub service: String,
    /// `ts` del último mensaje consumido.
    #[serde(default)]
    pub last_ts: i64,
    /// Id del último mensaje consumido (desempata a igual `ts`).
    #[serde(default)]
    pub last_id: String,
    /// Epoch de la última actualización del cursor.
    #[serde(default)]
    pub updated_at: i64,
    /// Huella del origen (p. ej. `tamaño:mtime` de una exportación).
    #[serde(default)]
    pub fingerprint: String,
}

/// Clave idempotente de una conversación.
pub fn conversation_key(service: &str, conversation_id: &str) -> String {
    format!("{}::{}", service, conversation_id)
}

/// Clave idempotente de un mensaje.
pub fn message_key(service: &str, conversation_id: &str, message_id: &str) -> String {
    format!("{}::{}::{}", service, conversation_id, message_id)
}

// ========================================================================= //
// 2. Catálogo de servicios
// ========================================================================= //

/// Cómo se llega al dato de cada servicio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceKind {
    /// Base de datos local del dispositivo (con root).
    LocalDb,
    /// Capa de notificaciones del sistema.
    Notifications,
    /// API del servicio con la sesión del usuario.
    WebApi,
    /// Exportación oficial (fichero en disco).
    Export,
    /// Motor de sync de IA existente (`crate::sync`), ruta local.
    AiLocal,
    /// Motor de sync de IA existente (`crate::sync_web`), ruta web.
    AiWeb,
}

/// Ficha de un servicio: de dónde salen los datos, con qué esquema, hasta
/// dónde se llega y qué falta.
#[derive(Clone, Copy, Debug)]
pub struct ServiceSpec {
    /// Identificador del servicio.
    pub service: &'static str,
    /// Nombre para la UI.
    pub label: &'static str,
    /// Vía de acceso.
    pub kind: ServiceKind,
    /// Origen de los datos.
    pub origin: Origin,
    /// Nivel de acceso **máximo** alcanzable con lo medido hoy.
    pub access: AccessLevel,
    /// Dónde están los datos (ruta exacta en el móvil o endpoint).
    pub data_source: &'static str,
    /// Esquema real medido (tablas/columnas o forma del JSON).
    pub schema: &'static str,
    /// Cómo se accede.
    pub route: &'static str,
    /// Qué falta para llegar al nivel completo.
    pub missing: &'static str,
    /// Comandos EXACTOS de medición en el móvil.
    pub measure: &'static [&'static str],
}

/// Catálogo de servicios.
///
/// Los servicios de IA que ya existen en `crate::sync` **no se duplican**:
/// aquí sólo se declara su ficha y su nivel, y el pull se hace llamando a
/// `sync::run_sync` / `sync_web::web_pull`.  Grok es el único añadido.
pub fn catalog() -> &'static [ServiceSpec] {
    &[
        // ------------------------------------------------------------- chat --
        ServiceSpec {
            service: "whatsapp",
            label: "WhatsApp",
            kind: ServiceKind::LocalDb,
            origin: Origin::LocalDb,
            access: AccessLevel::Metadatos,
            data_source: "/data/data/com.whatsapp/databases/msgstore.db (+ .crypt15) y \
                          /data/data/com.whatsapp/databases/wa.db",
            schema: "msgstore.db: SQLCipher (cabecera no-SQLite) -> NO legible. \
                     wa.db: tabla de contactos (jid, display_name) y de chats \
                     (jid, unread_count, message_count, last_message_time)",
            route: "root (su -mm -c) + dumpsys notification --noredact",
            missing: "el texto de los mensajes: la clave AES vive en el \
                      AndroidKeyStore y no es exportable; hace falta hook \
                      in-process (Frida) o la exportación oficial. NO se \
                      intenta descifrar.",
            measure: &[
                "su -mm -c 'ls -l /data/data/com.whatsapp/databases/'",
                "su -mm -c 'head -c 16 /data/data/com.whatsapp/databases/msgstore.db | xxd'",
                "su -mm -c 'head -c 16 /data/data/com.whatsapp/databases/wa.db | xxd'",
                "su -mm -c 'dumpsys notification --noredact | grep -A3 com.whatsapp'",
            ],
        },
        ServiceSpec {
            service: "whatsapp_business",
            label: "WhatsApp Business",
            kind: ServiceKind::LocalDb,
            origin: Origin::LocalDb,
            access: AccessLevel::Metadatos,
            data_source: "/data/data/com.whatsapp.w4b/databases/{msgstore.db,wa.db}",
            schema: "igual que WhatsApp (mismo motor, otro paquete)",
            route: "root + dumpsys notification --noredact",
            missing: "mismo límite de cifrado que WhatsApp",
            measure: &[
                "su -mm -c 'ls -l /data/data/com.whatsapp.w4b/databases/'",
                "su -mm -c 'dumpsys notification --noredact | grep -A3 com.whatsapp.w4b'",
            ],
        },
        ServiceSpec {
            service: "telegram",
            label: "Telegram",
            kind: ServiceKind::LocalDb,
            origin: Origin::LocalDb,
            access: AccessLevel::Completo,
            data_source: "/data/data/org.telegram.messenger/databases/*.db",
            schema: "SQLite SIN cifrar: messages(uid, chat_id, from_id, date, \
                     message), chats/dialogs, users, info",
            route: "root: se copia la BD (con -wal/-shm) a un snapshot y se \
                    lee con SQLite",
            missing: "nada para el texto; sólo el formato de `message` cambia \
                      entre versiones (se detecta por columnas)",
            measure: &[
                "su -mm -c 'ls -l /data/data/org.telegram.messenger/databases/'",
                "su -mm -c 'cp /data/data/org.telegram.messenger/databases/*.db /data/local/tmp/'",
                "sqlite3 /data/local/tmp/msgstore.db '.tables'",
                "sqlite3 /data/local/tmp/msgstore.db 'PRAGMA table_info(messages)'",
                "sqlite3 /data/local/tmp/msgstore.db 'PRAGMA table_info(chats)'",
            ],
        },
        ServiceSpec {
            service: "signal",
            label: "Signal",
            kind: ServiceKind::Notifications,
            origin: Origin::Notifications,
            access: AccessLevel::CifradoNoDescifrado,
            data_source: "/data/data/org.thoughtcrime.securesms/app_database/*.db \
                          (SQLCipher) + dumpsys notification --noredact",
            schema: "signal.db va cifrado con SQLCipher (cabecera no-SQLite): \
                     la lista de tablas NO es legible. Lo legible es la capa \
                     de notificaciones (pkg, android.title, android.text)",
            route: "root + dumpsys notification --noredact",
            missing: "TODO el contenido de los mensajes. NO se intenta romper \
                      el cifrado de Signal: la clave está atada al dispositivo.",
            measure: &[
                "su -mm -c 'ls -l /data/data/org.thoughtcrime.securesms/app_database/'",
                "su -mm -c 'head -c 16 /data/data/org.thoughtcrime.securesms/app_database/signal.db | xxd'",
                "su -mm -c 'dumpsys notification --noredact | grep -A3 org.thoughtcrime.securesms'",
            ],
        },
        ServiceSpec {
            service: "discord",
            label: "Discord",
            kind: ServiceKind::WebApi,
            origin: Origin::WebApi,
            access: AccessLevel::Completo,
            data_source: "API oficial con el token del usuario \
                          (discord.com/api/v9) o exportación oficial (messages.json)",
            schema: "API: {channels:[{id,name,type,guild:{name}}]} y \
                     {messages:[{id,timestamp,content,author:{name,username}}]}. \
                     Export: {\"channels\":[{\"id\",\"name\",\"messages\":[...]}]} \
                     (mismo formato de mensaje)",
            route: "curl del dispositivo con cabecera Authorization (token en \
                    fichero 0600) o ingesta del messages.json de la exportación",
            missing: "el token: hay que pegaarlo una vez (nunca se guarda en \
                      el repo). Sin token, sólo queda la exportación oficial.",
            measure: &[
                "su -mm -c 'ls -l /data/data/com.discord/app_localstorage/leveldb'",
                "su -mm -c 'curl -s -H \"Authorization: <TOKEN>\" https://discord.com/api/v9/users/@me/channels | head -c 400'",
                "su -mm -c 'find /sdcard -name messages.json 2>/dev/null'",
            ],
        },
        ServiceSpec {
            service: "grok",
            label: "Grok (xAI)",
            kind: ServiceKind::WebApi,
            origin: Origin::WebApi,
            access: AccessLevel::Completo,
            data_source: "web de Grok: sesión del navegador/app (cookies) o \
                          exportación oficial",
            schema: "NO VERIFICADO: los endpoints internos de grok.com hay \
                     que medirlos en el móvil (ver `measure`). La API \
                     pública de xAI es https://api.x.ai/ pero NO da el \
                     historial de chat.",
            route: "curl del dispositivo con las cookies de la sesión \
                    (hook de Frida `assets/frida_chrome_cookies.js` o la \
                    exportación oficial)",
            missing: "el paquete Android verificado es `ai.x.grok` \
                      (id previo: `com.x.ai`); el historial local de la app \
                      no está mapeado todavía",
            measure: &[
                "su -mm -c 'pm list packages | grep -i grok'",
                "su -mm -c 'ls -l /data/data/ai.x.grok/app_webview/Default/'",
                "su -mm -c 'dumpsys activity activities | grep -i grok'",
                "su -mm -c 'curl -s -o /dev/null -w \"%{http_code}\" https://grok.com/'",
            ],
        },
        // ------------------------------------------------------- apps de IA --
        ServiceSpec {
            service: "chatgpt",
            label: "ChatGPT",
            kind: ServiceKind::AiLocal,
            origin: Origin::LocalDb,
            access: AccessLevel::Completo,
            data_source: "/data/data/com.openai.chatgpt/files/sessions/*/default/databases/*_conversations.db",
            schema: "DBMessageChunk.chunk.content.content (medido)",
            route: "crate::sync::run_sync (extractor Python embebido)",
            missing: "nada (ya resuelto); Grok es el que se ha añadido",
            measure: &["su -mm -c 'ls /data/data/com.openai.chatgpt/files/sessions/*/default/databases/'"],
        },
        ServiceSpec {
            service: "claude",
            label: "Claude",
            kind: ServiceKind::AiLocal,
            origin: Origin::LocalDb,
            access: AccessLevel::Completo,
            data_source: "/data/data/com.anthropic.claude/databases/*conversation_snapshots*.db",
            schema: "snapshot_messages.body (JSON con blocks[].content.block.text/thinking) (medido)",
            route: "crate::sync::run_sync",
            missing: "nada",
            measure: &["su -mm -c 'ls /data/data/com.anthropic.claude/databases/'"],
        },
        ServiceSpec {
            service: "gemini",
            label: "Gemini",
            kind: ServiceKind::AiWeb,
            origin: Origin::WebApi,
            access: AccessLevel::Metadatos,
            data_source: "servidor (la app Android NO guarda historial local)",
            schema: "NO hay store local: la app no persiste el chat. Sólo la \
                     web (gemini.google.com) y la exportación de Takeout",
            route: "crate::sync_web::web_pull (necesita la cookie __Secure-1PSID \
                    de la cuenta real)",
            missing: "la cookie de la cuenta real (la sesión cosechada es de \
                      invitado)",
            measure: &["su -mm -c 'ls /data/data/com.google.android.apps.bard/databases/'"],
        },
        ServiceSpec {
            service: "deepseek",
            label: "DeepSeek",
            kind: ServiceKind::AiLocal,
            origin: Origin::LocalDb,
            access: AccessLevel::Completo,
            data_source: "/data/data/com.deepseek.chat/databases/deepseek_chat_*.db",
            schema: "chat_session_list + chat_session_messages_<id> (medido)",
            route: "crate::sync::run_sync",
            missing: "nada",
            measure: &["su -mm -c 'ls /data/data/com.deepseek.chat/databases/'"],
        },
        ServiceSpec {
            service: "kimi",
            label: "Kimi",
            kind: ServiceKind::AiLocal,
            origin: Origin::LocalDb,
            access: AccessLevel::Completo,
            data_source: "/data/data/com.moonshot.kimichat/databases/kimi_*_chat.db",
            schema: "DBChatMessage.extra.blocks[].text / .think (medido)",
            route: "crate::sync::run_sync",
            missing: "nada",
            measure: &["su -mm -c 'ls /data/data/com.moonshot.kimichat/databases/'"],
        },
        ServiceSpec {
            service: "mistral",
            label: "Mistral",
            kind: ServiceKind::AiLocal,
            origin: Origin::LocalDb,
            access: AccessLevel::Completo,
            data_source: "/data/data/ai.mistral.chat/databases/*.db",
            schema: "genérico (el extractor busca tablas con estructura de \
                     conversación; esquema exacto NO medido)",
            route: "crate::sync::run_sync",
            missing: "el esquema exacto de la app (NO VERIFICADO: hay que \
                      medirlo)",
            measure: &["su -mm -c 'ls /data/data/ai.mistral.chat/databases/'"],
        },
        ServiceSpec {
            service: "perplexity",
            label: "Perplexity",
            kind: ServiceKind::AiLocal,
            origin: Origin::LocalDb,
            access: AccessLevel::Completo,
            data_source: "/data/data/ai.perplexity.app.android/databases/*.db",
            schema: "genérico (esquema exacto NO medido)",
            route: "crate::sync::run_sync",
            missing: "el esquema exacto de la app (NO VERIFICADO)",
            measure: &["su -mm -c 'ls /data/data/ai.perplexity.app.android/databases/'"],
        },
        ServiceSpec {
            service: "copilot",
            label: "Copilot",
            kind: ServiceKind::AiLocal,
            origin: Origin::LocalDb,
            access: AccessLevel::Completo,
            data_source: "/data/data/com.microsoft.copilot/databases/*.db",
            schema: "genérico (esquema exacto NO medido)",
            route: "crate::sync::run_sync",
            missing: "el esquema exacto de la app (NO VERIFICADO)",
            measure: &["su -mm -c 'ls /data/data/com.microsoft.copilot/databases/'"],
        },
        ServiceSpec {
            service: "poe",
            label: "Poe",
            kind: ServiceKind::AiLocal,
            origin: Origin::LocalDb,
            access: AccessLevel::Completo,
            data_source: "/data/data/com.poe.android/databases/*.db",
            schema: "genérico (esquema exacto NO medido)",
            route: "crate::sync::run_sync",
            missing: "el esquema exacto de la app (NO VERIFICADO)",
            measure: &["su -mm -c 'ls /data/data/com.poe.android/databases/'"],
        },
    ]
}

/// Ficha de un servicio por nombre.
pub fn spec(service: &str) -> Option<&'static ServiceSpec> {
    catalog().iter().find(|s| s.service == service)
}

/// Nombres de todos los servicios del catálogo.
pub fn service_names() -> Vec<&'static str> {
    catalog().iter().map(|s| s.service).collect()
}

// ========================================================================= //
// 3. Almacén, cursores y merge
// ========================================================================= //

/// Directorio del almacén de chats (independiente del de `sync.rs`).
pub fn store_dir() -> PathBuf {
    PathBuf::from(crate::cockpit_server::state_dir()).join("sync_chats")
}

/// Fichero de cursores.
pub fn cursor_path() -> PathBuf {
    store_dir().join("cursors.json")
}

/// Fichero del token de Discord (0600, NUNCA en el repo).
pub fn discord_token_path() -> PathBuf {
    store_dir().join("discord_token")
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Carga el almacén (clave `servicio::conversation_id` → conversación).
pub fn load_store() -> BTreeMap<String, Conversation> {
    let mut map: BTreeMap<String, Conversation> = BTreeMap::new();
    let dir = store_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return map;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ndjson") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(c) = serde_json::from_str::<Conversation>(line) else {
                continue;
            };
            if c.service.is_empty() || c.conversation_id.is_empty() {
                continue;
            }
            map.insert(c.key(), c);
        }
    }
    map
}

/// Guarda el almacén (un NDJSON por servicio).
pub fn save_store(store: &BTreeMap<String, Conversation>) -> Result<(), String> {
    let dir = store_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir {:?}: {}", dir, e))?;
    let mut by_service: BTreeMap<String, Vec<&Conversation>> = BTreeMap::new();
    for c in store.values() {
        by_service.entry(c.service.clone()).or_default().push(c);
    }
    for (svc, list) in by_service {
        let mut buf = String::new();
        for c in list {
            buf.push_str(&serde_json::to_string(c).unwrap_or_default());
            buf.push('\n');
        }
        let path = dir.join(format!("{}.ndjson", svc));
        let tmp = dir.join(format!(".{}.tmp", svc));
        fs::write(&tmp, buf.as_bytes()).map_err(|e| format!("write {:?}: {}", tmp, e))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename: {}", e))?;
    }
    Ok(())
}

/// Fusiona una conversación entrante con la que ya había en el almacén.
///
/// Idempotencia a nivel de **mensaje**: la clave es
/// `servicio::conversation_id::message_id`.  Un mensaje repetido se cuenta
/// UNA vez; el texto nuevo replaces al viejo (misma clave).
pub fn merge_conversation(existing: Option<Conversation>, incoming: Conversation) -> Conversation {
    let Some(mut prev) = existing else {
        let mut c = incoming;
        sort_messages(&mut c.messages);
        return c;
    };

    // Los mensajes se indexan por id (clave idempotente).
    let mut index: BTreeMap<String, ChatMessage> = BTreeMap::new();
    for m in prev.messages.drain(..) {
        index.insert(m.id.clone(), m);
    }
    for m in incoming.messages.into_iter() {
        index.insert(m.id.clone(), m);
    }
    let mut messages: Vec<ChatMessage> = index.into_values().collect();
    sort_messages(&mut messages);

    // El título gana si el nuevo no viene vacío.
    if !incoming.title.trim().is_empty() {
        prev.title = incoming.title;
    }
    // El nivel de acceso sólo sube (y el origen se recalcula con él).
    if incoming.access > prev.access {
        prev.access = incoming.access;
        prev.origin = incoming.origin;
    }
    if !incoming.source.trim().is_empty() {
        prev.source = incoming.source;
    }
    if !incoming.note.trim().is_empty() {
        prev.note = incoming.note;
    }
    prev.updated_at = prev.updated_at.max(incoming.updated_at);
    prev.messages = messages;
    prev
}

/// Ordena los mensajes por `(ts, id)`.
fn sort_messages(msgs: &mut [ChatMessage]) {
    msgs.sort_by(|a, b| {
        a.ts.cmp(&b.ts).then_with(|| a.id.cmp(&b.id))
    });
}

/// Resultado de un merge.
#[derive(Clone, Copy, Debug, Default)]
pub struct MergeStats {
    /// Conversaciones nuevas.
    pub added: usize,
    /// Conversaciones ya existentes.
    pub updated: usize,
    /// Mensajes nuevos (ids que no estaban).
    pub new_messages: usize,
    /// Mensajes repetidos (idempotencia).
    pub repeated_messages: usize,
}

/// Fusiona un lote de conversaciones en el almacén.
pub fn merge_all(
    store: &mut BTreeMap<String, Conversation>,
    incoming: Vec<Conversation>,
) -> MergeStats {
    let mut stats = MergeStats::default();
    for conv in incoming {
        let key = conv.key();
        let prev = store.get(&key);
        let before: Vec<String> = prev
            .map(|p| p.messages.iter().map(|m| m.id.clone()).collect())
            .unwrap_or_default();
        let merged = merge_conversation(prev.cloned(), conv);
        for m in &merged.messages {
            if before.contains(&m.id) {
                stats.repeated_messages += 1;
            } else {
                stats.new_messages += 1;
            }
        }
        if prev.is_some() {
            stats.updated += 1;
        } else {
            stats.added += 1;
        }
        store.insert(key, merged);
    }
    stats
}

/// Carga los cursores por servicio.
pub fn load_cursors() -> BTreeMap<String, Cursor> {
    let mut map = BTreeMap::new();
    let Ok(text) = fs::read_to_string(cursor_path()) else {
        return map;
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return map;
    };
    if let Some(obj) = v.as_object() {
        for (svc, cur) in obj {
            if let Ok(c) = serde_json::from_value::<Cursor>(cur.clone()) {
                map.insert(svc.clone(), c);
            }
        }
    }
    map
}

/// Cursor de un servicio (vacío si no hay).
pub fn cursor_for(service: &str) -> Cursor {
    load_cursors()
        .get(service)
        .cloned()
        .unwrap_or_else(|| Cursor {
            service: service.to_string(),
            ..Default::default()
        })
}

/// Guarda un cursor.
pub fn save_cursor(cursor: &Cursor) -> Result<(), String> {
    let mut cursors = load_cursors();
    cursors.insert(cursor.service.clone(), cursor.clone());
    let dir = store_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir {:?}: {}", dir, e))?;
    let path = cursor_path();
    let tmp = dir.join(".cursors.tmp");
    fs::write(
        &tmp,
        serde_json::to_string_pretty(&cursors).unwrap_or_default(),
    )
    .map_err(|e| format!("write {:?}: {}", tmp, e))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename: {}", e))?;
    Ok(())
}

/// ¿Es un mensaje nuevo frente al cursor? (incremental)
pub fn is_newer_than_cursor(ts: i64, id: &str, cursor: &Cursor) -> bool {
    if cursor.last_ts == 0 && cursor.last_id.is_empty() {
        return true;
    }
    if ts > cursor.last_ts {
        return true;
    }
    ts == cursor.last_ts && id > cursor.last_id.as_str()
}

/// Calcula el cursor que corresponde a un lote de mensajes.
pub fn advance_cursor(service: &str, cursor: &Cursor, messages: &[ChatMessage]) -> Cursor {
    let mut next = cursor.clone();
    next.service = service.to_string();
    for m in messages {
        if m.ts > next.last_ts || (m.ts == next.last_ts && m.id > next.last_id) {
            next.last_ts = m.ts;
            next.last_id = m.id.clone();
        }
    }
    next.updated_at = now_epoch();
    next
}

/// Hash corto y determinista (para ids de mensaje sintetizados).
pub fn short_hash(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let d = h.finalize();
    d.iter().take(6).map(|b| format!("{:02x}", b)).collect()
}

/// Id determinista para un mensaje sin id propio.
pub fn synthetic_message_id(ts: i64, author: &str, text: &str) -> String {
    format!("{}-{}-{}", ts, short_hash(author), short_hash(text))
}

// ========================================================================= //
// 4. Clasificación de bases de datos (cifrado honesto)
// ========================================================================= //

/// Estado de un fichero de base de datos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DbState {
    /// SQLite en claro: se puede leer.
    Plain,
    /// Cifrado (SQLCipher) o corrupto: **no** se puede leer.
    Encrypted,
    /// Fichero vacío.
    Empty,
}

/// Clasifica un fichero de BD por su cabecera.
///
/// Un SQLite en claro empieza por `SQLite format 3\0`.  Un fichero cifrado
/// con SQLCipher **no** tiene esa cabecera: es exactamente el caso de
/// `msgstore.db` (WhatsApp) y `signal.db` (Signal).  En ese caso el estado
/// honesto es `cifrado_no_descifrado` y **no** se inventa ningún mensaje.
pub fn classify_db(bytes: &[u8]) -> DbState {
    if bytes.is_empty() {
        return DbState::Empty;
    }
    if bytes.starts_with(b"SQLite format 3\0") {
        return DbState::Plain;
    }
    DbState::Encrypted
}

/// Explicación estándar del estado de cifrado (sin adivinar).
pub const ENCRYPTION_REASON: &str = "El almacén está cifrado (SQLCipher): la cabecera del \
    fichero no es SQLite, así que no se puede leer el contenido sin la clave. La clave vive \
    en el AndroidKeyStore y no es exportable, ni siquiera con root. NO se intenta descifrar: \
    el nivel de acceso es `cifrado_no_descifrado` y la lista de mensajes queda vacía.";

/// Resultado de un adaptador.
#[derive(Clone, Debug, Default)]
pub struct AdapterReport {
    /// Servicio.
    pub service: String,
    /// Origen de los datos.
    pub origin: Origin,
    /// Nivel de acceso alcanzado.
    pub access: AccessLevel,
    /// Estado: `ok`, `sin_acceso`, `cifrado`, `parcial`, `error`.
    pub status: String,
    /// Conversaciones encontradas.
    pub conversations: Vec<Conversation>,
    /// Esquema real detectado (JSON).
    pub schema: Value,
    /// Notas (qué falta, por qué no se puede más).
    pub notes: Vec<String>,
    /// Errores reales (tal cual).
    pub errors: Vec<String>,
    /// Origen exacto (ruta/endpoint).
    pub source: String,
    /// Datos extra REALES del servicio (contactos, chats, ...).
    pub data: Value,
}

impl AdapterReport {
    /// Reporte de error honesto (nada inventado).
    pub fn error(service: &str, msg: &str) -> AdapterReport {
        AdapterReport {
            service: service.to_string(),
            origin: Origin::LocalDb,
            access: AccessLevel::Metadatos,
            status: "error".to_string(),
            conversations: vec![],
            schema: Value::Null,
            notes: vec![],
            errors: vec![msg.to_string()],
            source: String::new(),
            data: Value::Null,
        }
    }

    /// Reporte sin acceso (no hay dispositivo, no hay token, ...).
    pub fn sin_acceso(service: &str, origin: Origin, msg: &str) -> AdapterReport {
        AdapterReport {
            service: service.to_string(),
            origin,
            access: AccessLevel::Metadatos,
            status: "sin_acceso".to_string(),
            conversations: vec![],
            schema: Value::Null,
            notes: vec![msg.to_string()],
            errors: vec![],
            source: String::new(),
            data: Value::Null,
        }
    }

    /// Reporte de almacén cifrado.
    pub fn cifrado(service: &str, origin: Origin, source: &str) -> AdapterReport {
        AdapterReport {
            service: service.to_string(),
            origin,
            access: AccessLevel::CifradoNoDescifrado,
            status: "cifrado".to_string(),
            conversations: vec![],
            schema: json!({ "cifrado": true, "cabecera": "no-SQLite" }),
            notes: vec![ENCRYPTION_REASON.to_string()],
            errors: vec![],
            source: source.to_string(),
            data: Value::Null,
        }
    }

    /// Número de mensajes verificados.
    pub fn messages(&self) -> usize {
        self.conversations
            .iter()
            .map(|c| c.verified_messages())
            .sum()
    }

    /// Convierte el reporte a JSON (el formato que ve la web y el MCP).
    pub fn to_value(&self) -> Value {
        json!({
            "service": self.service,
            "origin": self.origin.as_str(),
            "access": self.access.as_str(),
            "status": self.status,
            "source": self.source,
            "conversations": self.conversations.len(),
            "messages": self.messages(),
            "schema": self.schema,
            "notes": self.notes,
            "errors": self.errors,
            "data": self.data,
            "items": self.conversations,
        })
    }
}

// ========================================================================= //
// 5. Utilidades de SQLite y de texto
// ========================================================================= //

/// Abre una BD en sólo lectura.
///
/// Si la BD está en modo WAL y no se puede crear el `-shm`, se reintenta en
/// lectura-escritura (mismo criterio que `crate::messaging`).  OJO: sólo es
/// inocuo porque **siempre** se abre sobre una copia local (la del
/// dispositivo se copia antes a un temporal).
fn open_ro(path: &Path) -> Result<Connection, String> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI;
    match Connection::open_with_flags(path, flags) {
        Ok(c) => Ok(c),
        Err(e) => match Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE) {
            Ok(c) => Ok(c),
            Err(e2) => Err(format!("{} / {}", e, e2)),
        },
    }
}

/// Escribe unos bytes en un temporal y devuelve la ruta (para abrir una BD
/// copiada del dispositivo).
pub fn temp_db_file(bytes: &[u8], tag: &str) -> Result<PathBuf, String> {
    let mut p = std::env::temp_dir();
    p.push(format!("md_sync_chats_{}_{}.db", tag, short_hash(&bytes.len().to_string())));
    fs::write(&p, bytes).map_err(|e| format!("write {:?}: {}", p, e))?;
    Ok(p)
}

/// Elige la primera columna que coincide (sin distinguir mayúsculas).
fn pick_col(cols: &[String], candidates: &[&str]) -> Option<String> {
    for c in candidates {
        if let Some(found) = cols.iter().find(|x| x.eq_ignore_ascii_case(c)) {
            return Some(found.clone());
        }
    }
    None
}

/// Identificador SQL seguro.
fn ident(name: &str) -> String {
    let clean: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    format!("\"{}\"", clean)
}

/// Valor de celda como texto (los blobs pasan por el decodificador de
/// `messaging`, que conoce el formato binario de Telegram).
fn cell_text(row: &rusqlite::Row<'_>, idx: usize) -> Option<String> {
    use rusqlite::types::ValueRef;
    match row.get_ref(idx) {
        Ok(ValueRef::Null) => None,
        Ok(ValueRef::Integer(i)) => Some(i.to_string()),
        Ok(ValueRef::Real(f)) => Some(f.to_string()),
        Ok(ValueRef::Text(t)) => Some(String::from_utf8_lossy(t).into_owned()),
        Ok(ValueRef::Blob(b)) => crate::messaging::decode_message_blob(b),
        Err(_) => None,
    }
}

/// Valor de celda como entero.
fn cell_i64(row: &rusqlite::Row<'_>, idx: usize) -> Option<i64> {
    use rusqlite::types::ValueRef;
    match row.get_ref(idx) {
        Ok(ValueRef::Integer(i)) => Some(i),
        Ok(ValueRef::Real(f)) => Some(f as i64),
        Ok(ValueRef::Text(t)) => String::from_utf8_lossy(t).trim().parse().ok(),
        _ => None,
    }
}

/// Convierte una marca ISO-8601 a epoch (UTC).
///
/// Discord usa `2024-01-02T03:04:05.123+00:00`.  No hay dependencia de
/// fecha en el crate, así que se usa el cálculo civil de `messaging`.
pub fn parse_iso8601(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let year: i64 = s.get(0..4)?.parse().ok()?;
    let month: u32 = s.get(5..7)?.parse().ok()?;
    let day: u32 = s.get(8..10)?.parse().ok()?;
    let hour: i64 = s.get(11..13)?.parse().ok()?;
    let minute: i64 = s.get(14..16)?.parse().ok()?;
    let second: i64 = s.get(17..19)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days = crate::messaging::days_from_civil(year, month, day);
    let mut ts = days * 86_400 + hour * 3600 + minute * 60 + second;
    let tail = &s[19..];
    if let Some(pos) = tail.rfind(|c| c == '+' || c == '-') {
        let off = &tail[pos..];
        if off.len() >= 6 {
            let sign: i64 = if off.starts_with('-') { -1 } else { 1 };
            let hh: i64 = off.get(1..3)?.parse().ok()?;
            let mm: i64 = off.get(4..6)?.parse().ok()?;
            ts -= sign * (hh * 3600 + mm * 60);
        }
    }
    Some(ts)
}

// ========================================================================= //
// 6. Adaptador: Telegram (BD local SIN cifrar)
// ========================================================================= //

/// Lee las conversaciones de una BD de Telegram.
///
/// El cursor se empuja **al SQL**: `WHERE date > <last_ts>`, así que la
/// segunda pasada no vuelve a traer los mensajes ya consumidos.
pub fn telegram_from_path(path: &Path, cursor: &Cursor, limit: usize) -> AdapterReport {
    let conn = match open_ro(path) {
        Ok(c) => c,
        Err(e) => {
            let mut r = AdapterReport::error("telegram", &format!("no se pudo abrir: {}", e));
            r.source = path.to_string_lossy().into_owned();
            return r;
        }
    };
    telegram_from_conn(&conn, cursor, limit, &path.to_string_lossy())
}

/// Lee las conversaciones de una BD de Telegram desde sus bytes.
pub fn telegram_from_bytes(bytes: &[u8], cursor: &Cursor, limit: usize) -> AdapterReport {
    match classify_db(bytes) {
        DbState::Empty => {
            let mut r = AdapterReport::error("telegram", "fichero vacío");
            r.status = "sin_acceso".to_string();
            return r;
        }
        DbState::Encrypted => return AdapterReport::cifrado("telegram", Origin::LocalDb, "bytes"),
        DbState::Plain => {}
    }
    let path = match temp_db_file(bytes, "telegram") {
        Ok(p) => p,
        Err(e) => return AdapterReport::error("telegram", &e),
    };
    let out = telegram_from_path(&path, cursor, limit);
    let _ = fs::remove_file(&path);
    out
}

/// Lector de Telegram sobre una conexión abierta.
fn telegram_from_conn(
    conn: &Connection,
    cursor: &Cursor,
    limit: usize,
    source: &str,
) -> AdapterReport {
    let schema = crate::messaging::detect_telegram_schema(conn);
    let schema_json = json!({
        "tables": schema.tables,
        "messages_table": schema.messages_table,
        "messages_columns": schema.message_columns,
        "chats_table": schema.chats_table,
        "chats_columns": schema.chat_columns,
        "users_table": schema.users_table,
        "info_table": schema.info_table,
        "text_column": schema.text_col,
        "date_column": schema.date_col,
        "from_column": schema.from_id_col,
    });

    let Some(messages_table) = schema.messages_table.clone() else {
        let mut r = AdapterReport::error("telegram", "no hay tabla de mensajes");
        r.schema = schema_json;
        r.status = "parcial".to_string();
        return r;
    };

    // Nombres de los chats (tabla chats/dialogs).
    let mut titles: BTreeMap<i64, String> = BTreeMap::new();
    if let Some(chats_table) = schema.chats_table.as_ref() {
        let key_col = pick_col(
            &schema.chat_columns,
            &["chat_id", "dialog_id", "peer_id", "_id", "id", "rowid"],
        )
        .map(|c| ident(&c))
        .unwrap_or_else(|| "\"rowid\"".to_string());
        let title_expr = schema
            .title_col
            .as_ref()
            .map(|c| ident(c))
            .unwrap_or_else(|| "NULL".to_string());
        let sql = format!("SELECT {}, {} FROM {}", key_col, title_expr, ident(chats_table));
        if let Ok(mut st) = conn.prepare(&sql) {
            if let Ok(mut rows) = st.query_map([], |row| {
                let k = cell_i64(row, 0);
                let t = cell_text(row, 1);
                Ok::<(Option<i64>, Option<String>), rusqlite::Error>((k, t))
            }) {
                while let Some(Ok((id, name))) = rows.next() {
                    if let Some(name) = name {
                        if !name.trim().is_empty() {
                            titles.insert(id.unwrap_or(0), name);
                        }
                    }
                }
            }
        }
    }

    // Nombres de los usuarios (tabla users): de ahí sale el autor real.
    let mut user_names: BTreeMap<i64, String> = BTreeMap::new();
    if let Some(users_table) = schema.users_table.as_ref() {
        let cols = crate::messaging::table_columns(conn, users_table);
        let id_col = pick_col(&cols, &["user_id", "uid", "id", "_id"]);
        let name_col = pick_col(
            &cols,
            &["first_name", "name", "display_name", "full_name", "username"],
        );
        if let (Some(id_col), Some(name_col)) = (id_col, name_col) {
            let sql = format!(
                "SELECT {}, {} FROM {} LIMIT 5000",
                ident(&id_col),
                ident(&name_col),
                ident(users_table)
            );
            if let Ok(mut st) = conn.prepare(&sql) {
                if let Ok(mut rows) = st.query_map([], |row| {
                    let id = cell_i64(row, 0);
                    let name = cell_text(row, 1);
                    Ok::<(Option<i64>, Option<String>), rusqlite::Error>((id, name))
                }) {
                    while let Some(Ok((id, name))) = rows.next() {
                        if let Some(name) = name {
                            if !name.trim().is_empty() {
                                user_names.insert(id.unwrap_or(0), name);
                            }
                        }
                    }
                }
            }
        }
    }

    // Identificador del propio usuario (para distinguir enviado/recibido).
    let self_id = crate::messaging::self_id_from_info(conn, &schema)
        .or_else(|| crate::messaging::self_id_from_users(conn, &schema));

    // Filas de mensajes, con el cursor aplicado en SQL.
    let id_expr = schema
        .id_col
        .as_ref()
        .map(|c| ident(c))
        .unwrap_or_else(|| "\"rowid\"".into());
    let chat_expr = schema
        .chat_id_col
        .as_ref()
        .map(|c| ident(c))
        .unwrap_or_else(|| "NULL".into());
    let from_expr = schema
        .from_id_col
        .as_ref()
        .map(|c| ident(c))
        .unwrap_or_else(|| "NULL".into());
    let date_expr = schema
        .date_col
        .as_ref()
        .map(|c| ident(c))
        .unwrap_or_else(|| "NULL".into());
    let text_expr = schema
        .text_col
        .as_ref()
        .map(|c| ident(c))
        .unwrap_or_else(|| "NULL".into());

    let where_clause = if schema.date_col.is_some() && cursor.last_ts > 0 {
        format!("WHERE {} > {}", date_expr, cursor.last_ts)
    } else {
        String::new()
    };
    let order_expr = schema
        .date_col
        .as_ref()
        .map(|c| ident(c))
        .unwrap_or_else(|| "\"rowid\"".into());
    let sql = format!(
        "SELECT {}, {}, {}, {}, {} FROM {}{} ORDER BY {} ASC LIMIT {}",
        id_expr,
        chat_expr,
        from_expr,
        date_expr,
        text_expr,
        ident(&messages_table),
        where_clause,
        order_expr,
        limit.clamp(1, 20_000)
    );

    let mut rows_raw: Vec<(Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<String>)> =
        vec![];
    if let Ok(mut st) = conn.prepare(&sql) {
        if let Ok(mut rows) = st.query_map([], |row| {
            Ok::<_, rusqlite::Error>((
                cell_i64(row, 0),
                cell_i64(row, 1),
                cell_i64(row, 2),
                cell_i64(row, 3),
                cell_text(row, 4),
            ))
        }) {
            while let Some(Ok(r)) = rows.next() {
                rows_raw.push(r);
            }
        }
    }

    // Sin `self_id` se infiere por patrón: el remitente más frecuente es el
    // usuario (es el que más mensajes envía).
    let self_id = match self_id {
        Some(v) => Some(v),
        None => {
            let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
            for (_, _, from, _, _) in &rows_raw {
                if let Some(f) = from {
                    *counts.entry(*f).or_insert(0) += 1;
                }
            }
            counts
                .iter()
                .max_by_key(|(k, v)| (**v, -(**k)))
                .map(|(k, _)| *k)
        }
    };

    // Agrupar por chat.
    let mut order: Vec<i64> = vec![];
    let mut groups: BTreeMap<i64, Vec<(Option<i64>, Option<i64>, Option<i64>, Option<String>)>> =
        BTreeMap::new();
    for (id, chat, from, date, text) in rows_raw {
        let chat_id = chat.unwrap_or(0);
        if !groups.contains_key(&chat_id) {
            order.push(chat_id);
        }
        groups.entry(chat_id).or_default().push((id, from, date, text));
    }

    let mut conversations: Vec<Conversation> = vec![];
    let mut total_messages = 0usize;
    for chat_id in order {
        let items = groups.remove(&chat_id).unwrap_or_default();
        let mut msgs: Vec<ChatMessage> = vec![];
        let mut updated_at = 0i64;
        for (id, from, date, text) in items {
            let ts = date.unwrap_or(0);
            let body = text.unwrap_or_default();
            if body.trim().is_empty() {
                continue;
            }
            let mid = match id {
                Some(i) => i.to_string(),
                None => synthetic_message_id(ts, &from.unwrap_or(0).to_string(), &body),
            };
            // Incremental también en Rust (por si el `date` no se pudo filtrar).
            if !is_newer_than_cursor(ts, &mid, cursor) {
                continue;
            }
            let author = match (from, self_id) {
                (Some(f), Some(s)) if f == s => "user".to_string(),
                (Some(f), _) => user_names
                    .get(&f)
                    .cloned()
                    .unwrap_or_else(|| format!("peer:{}", f)),
                (None, _) => "peer".to_string(),
            };
            updated_at = updated_at.max(ts);
            msgs.push(ChatMessage {
                id: mid,
                ts,
                author,
                text: body,
                kind: "texto".to_string(),
                verified: true,
            });
        }
        if msgs.is_empty() {
            continue;
        }
        sort_messages(&mut msgs);
        total_messages += msgs.len();
        let title = titles
            .get(&chat_id)
            .cloned()
            .unwrap_or_else(|| format!("chat {}", chat_id));
        conversations.push(Conversation {
            service: "telegram".to_string(),
            conversation_id: format!("chat_{}", chat_id),
            title,
            messages: msgs,
            origin: Origin::LocalDb,
            access: AccessLevel::Completo,
            updated_at,
            source: source.to_string(),
            note: String::new(),
        });
    }

    AdapterReport {
        service: "telegram".to_string(),
        origin: Origin::LocalDb,
        access: if total_messages > 0 {
            AccessLevel::Completo
        } else {
            AccessLevel::Metadatos
        },
        status: if total_messages > 0 { "ok" } else { "parcial" }.to_string(),
        conversations,
        schema: schema_json,
        notes: vec![],
        errors: vec![],
        source: source.to_string(),
        data: Value::Null,
    }
}

// ========================================================================= //
// 7. Adaptador: WhatsApp (metadatos + notificaciones)
// ========================================================================= //

/// Extrae contactos y chats de `wa.db` (que **no** va cifrado).
///
/// Son metadatos reales: quién tiene el usuario en la agenda y qué chats
/// hay con cuántos no leídos.  El **texto** de los mensajes NO sale de aquí.
pub fn whatsapp_metadata_from_bytes(bytes: &[u8], limit: usize) -> AdapterReport {
    if classify_db(bytes) == DbState::Encrypted {
        return AdapterReport::cifrado("whatsapp", Origin::LocalDb, "wa.db");
    }
    let path = match temp_db_file(bytes, "wa") {
        Ok(p) => p,
        Err(e) => return AdapterReport::error("whatsapp", &e),
    };
    let out = whatsapp_metadata_from_path(&path, limit);
    let _ = fs::remove_file(&path);
    out
}

/// Extrae contactos y chats de `wa.db` desde una ruta.
pub fn whatsapp_metadata_from_path(path: &Path, limit: usize) -> AdapterReport {
    let conn = match open_ro(path) {
        Ok(c) => c,
        Err(e) => {
            let mut r = AdapterReport::error("whatsapp", &format!("no se pudo abrir: {}", e));
            r.source = path.to_string_lossy().into_owned();
            return r;
        }
    };
    let tables = crate::messaging::table_names(&conn);
    let mut schema = vec![];
    let mut contacts_table: Option<String> = None;
    let mut chats_table: Option<String> = None;
    for t in &tables {
        let cols = crate::messaging::table_columns(&conn, t);
        schema.push(json!({ "name": t, "columns": cols }));
        let has_jid = cols.iter().any(|c| {
            let l = c.to_lowercase();
            l == "jid" || l.contains("jid") || l.contains("number") || l == "msisdn"
        });
        let has_name = cols.iter().any(|c| {
            let l = c.to_lowercase();
            l == "name" || l.contains("display_name") || l.contains("push_name") || l.contains("first_name")
        });
        let has_chat = cols.iter().any(|c| {
            let l = c.to_lowercase();
            l.contains("unread") || l.contains("message_count") || l.contains("chat_row")
        });
        if has_jid && has_name && contacts_table.is_none() {
            contacts_table = Some(t.clone());
        }
        if has_jid && has_chat && chats_table.is_none() {
            chats_table = Some(t.clone());
        }
    }

    let contacts = contacts_table
        .as_ref()
        .map(|t| rows_as_json(&conn, t, limit))
        .unwrap_or_default();
    let chats = chats_table
        .as_ref()
        .map(|t| rows_as_json(&conn, t, limit))
        .unwrap_or_default();

    // Los chats se exponen como conversaciones SIN mensajes: son metadatos.
    let mut conversations: Vec<Conversation> = vec![];
    if !chats.is_empty() {
        let mut msgs = vec![];
        let mut updated_at = 0i64;
        for c in &chats {
            let ts = c
                .get("last_message_time")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            updated_at = updated_at.max(ts);
            let jid = c.get("jid").and_then(|v| v.as_str()).unwrap_or("");
            let name = c
                .get("display_name")
                .or_else(|| c.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let unread = c
                .get("unread_count")
                .or_else(|| c.get("message_count"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            msgs.push(ChatMessage {
                id: format!("wa-chat-{}", jid),
                ts,
                author: if name.is_empty() { jid.to_string() } else { name.to_string() },
                text: format!("{} mensajes sin leer", unread),
                kind: "sistema".to_string(),
                // NO VERIFICADO: esto NO es el texto de un mensaje, es un
                // contador leído de la tabla de chats.
                verified: false,
            });
        }
        conversations.push(Conversation {
            service: "whatsapp".to_string(),
            conversation_id: "chats".to_string(),
            title: "WhatsApp (chats)".to_string(),
            messages: msgs,
            origin: Origin::LocalDb,
            access: AccessLevel::Metadatos,
            updated_at,
            source: path.to_string_lossy().into_owned(),
            note: "sólo metadatos (jid, no leídos): el texto de los mensajes \
                   va cifrado en msgstore.db"
                .to_string(),
        });
    }

    AdapterReport {
        service: "whatsapp".to_string(),
        origin: Origin::LocalDb,
        access: AccessLevel::Metadatos,
        status: "parcial".to_string(),
        conversations,
        schema: json!({
            "tables": schema,
            "contacts_table": contacts_table,
            "chats_table": chats_table,
            "contacts": contacts.len(),
        }),
        notes: vec![ENCRYPTION_REASON.to_string()],
        errors: vec![],
        source: path.to_string_lossy().into_owned(),
        data: json!({ "contacts": contacts, "chats": chats }),
    }
}

/// Contactos reales de `wa.db` (jid + nombre), tal cual están en la tabla.
///
/// `wa.db` **no** va cifrado: los contactos son metadatos legibles.  Lo que
/// no sale de aquí es el texto de los mensajes (eso vive en `msgstore.db`,
/// cifrado con SQLCipher).
pub fn whatsapp_contacts(path: &Path, limit: usize) -> Vec<Value> {
    let Ok(conn) = open_ro(path) else {
        return vec![];
    };
    let tables = crate::messaging::table_names(&conn);
    for t in &tables {
        let cols = crate::messaging::table_columns(&conn, t);
        let has_jid = cols.iter().any(|c| {
            let l = c.to_lowercase();
            l == "jid" || l.contains("jid") || l.contains("number") || l == "msisdn"
        });
        let has_name = cols.iter().any(|c| {
            let l = c.to_lowercase();
            l == "name" || l.contains("display_name") || l.contains("push_name") || l.contains("first_name")
        });
        if has_jid && has_name {
            return rows_as_json(&conn, t, limit);
        }
    }
    vec![]
}

/// Vuelca filas de una tabla como array de objetos JSON.
fn rows_as_json(conn: &Connection, table: &str, limit: usize) -> Vec<Value> {
    let sql = format!("SELECT * FROM {} LIMIT {}", ident(table), limit.clamp(1, 5000));
    let Ok(mut st) = conn.prepare(&sql) else {
        return vec![];
    };
    let cols: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
    let Ok(mut rows) = st.query_map([], |row| {
        let mut obj = serde_json::Map::new();
        for (i, c) in cols.iter().enumerate() {
            let v = match row.get_ref(i) {
                Ok(rusqlite::types::ValueRef::Null) => Value::Null,
                Ok(rusqlite::types::ValueRef::Integer(x)) => json!(x),
                Ok(rusqlite::types::ValueRef::Real(x)) => json!(x),
                Ok(rusqlite::types::ValueRef::Text(t)) => {
                    json!(String::from_utf8_lossy(t).chars().take(2000).collect::<String>())
                }
                Ok(rusqlite::types::ValueRef::Blob(b)) => {
                    json!(String::from_utf8_lossy(b).chars().take(2000).collect::<String>())
                }
                Err(_) => Value::Null,
            };
            obj.insert(c.clone(), v);
        }
        Ok::<Value, rusqlite::Error>(Value::Object(obj))
    }) else {
        return vec![];
    };
    let mut out = vec![];
    while let Some(Ok(r)) = rows.next() {
        out.push(r);
    }
    out
}

/// Convierte un volcado de `dumpsys notification` en conversaciones.
///
/// Lo que sale de aquí es **real** (el texto que el sistema ya mostraba) pero
/// NO es el historial del chat: por eso el origen es `notificaciones` y el
/// nivel `metadatos`.
pub fn notifications_conversations(
    service: &str,
    package: &str,
    dump: &str,
    cursor: &Cursor,
) -> Vec<Conversation> {
    let records = crate::messaging::parse_notification_dump(dump, &[package]);
    let mut by_title: BTreeMap<String, Vec<ChatMessage>> = BTreeMap::new();
    let mut order: Vec<String> = vec![];
    for r in records {
        let title = r.title.trim().to_string();
        let text = r.text.trim().to_string();
        if title.is_empty() && text.is_empty() {
            continue;
        }
        let ts = r.ts.unwrap_or(0);
        let body = if title.is_empty() {
            text.clone()
        } else if text.is_empty() {
            title.clone()
        } else {
            format!("{}: {}", title, text)
        };
        let id = format!("notif-{}-{}", ts, short_hash(&body));
        if !is_newer_than_cursor(ts, &id, cursor) {
            continue;
        }
        if !by_title.contains_key(&title) {
            order.push(title.clone());
        }
        by_title.entry(title.clone()).or_default().push(ChatMessage {
            id,
            ts,
            author: if title.is_empty() { "peer".to_string() } else { title.clone() },
            text: body,
            kind: "notificacion".to_string(),
            verified: true,
        });
    }
    order
        .into_iter()
        .filter_map(|title| {
            let mut msgs = by_title.remove(&title)?;
            if msgs.is_empty() {
                return None;
            }
            sort_messages(&mut msgs);
            let updated_at = msgs.iter().map(|m| m.ts).max().unwrap_or(0);
            Some(Conversation {
                service: service.to_string(),
                conversation_id: format!("notif:{}", short_hash(&title)),
                title: if title.is_empty() {
                    format!("{} (notificaciones)", service)
                } else {
                    title.clone()
                },
                messages: msgs,
                origin: Origin::Notifications,
                access: AccessLevel::Metadatos,
                updated_at,
                source: "dumpsys notification --noredact".to_string(),
                note: "texto de la notificación visible en el sistema: NO es \
                       el historial del chat"
                    .to_string(),
            })
        })
        .collect()
}

// ========================================================================= //
// 8. Adaptador: Signal (cifrado, sólo metadatos)
// ========================================================================= //

/// Estado del almacén de Signal.
///
/// Signal guarda TODO en SQLCipher.  Este lector **no** intenta descifrar:
/// comprueba la cabecera y devuelve el estado honesto.
pub fn signal_db_state(bytes: &[u8], source: &str) -> AdapterReport {
    match classify_db(bytes) {
        DbState::Encrypted => AdapterReport::cifrado("signal", Origin::LocalDb, source),
        DbState::Empty => {
            let mut r = AdapterReport::error("signal", "fichero vacío");
            r.status = "sin_acceso".to_string();
            r.source = source.to_string();
            r
        }
        DbState::Plain => {
            // Si algún día el esquema cambia, se informa en vez de inventar.
            let mut r = AdapterReport::error(
                "signal",
                "la cabecera es SQLite pero el esquema de Signal no está \
                 mapeado: no se inventa ningún mensaje",
            );
            r.status = "parcial".to_string();
            r.source = source.to_string();
            r
        }
    }
}

// ========================================================================= //
// 9. Adaptador: Discord (API con token + exportación oficial)
// ========================================================================= //

/// Token de Discord desde el fichero `0600` del `state_dir`.
pub fn read_discord_token() -> Option<String> {
    let txt = fs::read_to_string(discord_token_path()).ok()?;
    let t = txt.trim().to_string();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

/// Guarda el token de Discord en un fichero `0600` (nunca en el repo).
pub fn write_discord_token(token: &str) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("token vacío".to_string());
    }
    let dir = store_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir {:?}: {}", dir, e))?;
    let path = discord_token_path();
    fs::write(&path, token.as_bytes()).map_err(|e| format!("write {:?}: {}", path, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// URL de los canales del usuario.
pub fn discord_channels_url() -> String {
    "https://discord.com/api/v9/users/@me/channels".to_string()
}

/// URL de los mensajes de un canal (con `after` para el incremental).
pub fn discord_messages_url(channel_id: &str, after: &str) -> String {
    let mut url = format!(
        "https://discord.com/api/v9/channels/{}/messages?limit=100",
        channel_id
    );
    if !after.is_empty() {
        url.push_str(&format!("&after={}", after));
    }
    url
}

/// Parsea la exportación oficial de Discord (`messages.json`).
///
/// La exportación trae `{"channels":[{...,"messages":[...]}]}`; algunos
/// ficheros traen el array directamente.  Se aceptan ambos.
pub fn parse_discord_export(text: &str, cursor: &Cursor, limit: usize) -> AdapterReport {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return AdapterReport::error("discord", "el JSON de la exportación no se puede leer");
    };
    let channels = v
        .get("channels")
        .and_then(|c| c.as_array())
        .cloned()
        .or_else(|| v.as_array().cloned())
        .unwrap_or_default();
    let mut conversations: Vec<Conversation> = vec![];
    let mut total = 0usize;
    for ch in channels.iter().take(limit.clamp(1, 5000)) {
        let id = ch.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let name = ch.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let guild = ch
            .get("guild")
            .and_then(|g| g.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let title = if guild.is_empty() {
            name.to_string()
        } else {
            format!("{} / {}", guild, name)
        };
        let msgs = discord_messages_from_array(
            ch.get("messages").and_then(|m| m.as_array()),
            id,
            cursor,
        );
        total += msgs.len();
        if msgs.is_empty() {
            continue;
        }
        conversations.push(Conversation {
            service: "discord".to_string(),
            conversation_id: id.to_string(),
            title: if title.is_empty() {
                format!("canal {}", id)
            } else {
                title
            },
            messages: msgs,
            origin: Origin::Export,
            access: AccessLevel::Completo,
            updated_at: 0,
            source: "export:messages.json".to_string(),
            note: String::new(),
        });
    }
    for c in conversations.iter_mut() {
        c.updated_at = c.messages.iter().map(|m| m.ts).max().unwrap_or(0);
    }
    AdapterReport {
        service: "discord".to_string(),
        origin: Origin::Export,
        access: if total > 0 {
            AccessLevel::Completo
        } else {
            AccessLevel::Metadatos
        },
        status: if total > 0 { "ok" } else { "parcial" }.to_string(),
        conversations,
        schema: json!({ "channels": channels.len(), "formato": "channels[].messages[]" }),
        notes: vec![],
        errors: vec![],
        source: "export:messages.json".to_string(),
        data: Value::Null,
    }
}

/// Parsea la respuesta de la API de Discord (`/channels/{id}/messages`).
pub fn parse_discord_api_messages(
    body: &str,
    channel_id: &str,
    channel_name: &str,
    cursor: &Cursor,
) -> AdapterReport {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return AdapterReport::error("discord", "la respuesta de la API no es JSON");
    };
    let arr = v.as_array().cloned().unwrap_or_default();
    let msgs = discord_messages_from_array(Some(&arr), channel_id, cursor);
    let total = msgs.len();
    let mut conversations = vec![];
    if total > 0 {
        let updated_at = msgs.iter().map(|m| m.ts).max().unwrap_or(0);
        conversations.push(Conversation {
            service: "discord".to_string(),
            conversation_id: channel_id.to_string(),
            title: if channel_name.is_empty() {
                format!("canal {}", channel_id)
            } else {
                channel_name.to_string()
            },
            messages: msgs,
            origin: Origin::WebApi,
            access: AccessLevel::Completo,
            updated_at,
            source: format!("discord.com/api/v9/channels/{}/messages", channel_id),
            note: String::new(),
        });
    }
    AdapterReport {
        service: "discord".to_string(),
        origin: Origin::WebApi,
        access: if total > 0 {
            AccessLevel::Completo
        } else {
            AccessLevel::Metadatos
        },
        status: if total > 0 { "ok" } else { "parcial" }.to_string(),
        conversations,
        schema: json!({ "formato": "discord api v9 messages[]" }),
        notes: vec![],
        errors: vec![],
        source: format!("discord.com/api/v9/channels/{}/messages", channel_id),
        data: Value::Null,
    }
}

/// Convierte el array de mensajes de Discord en mensajes unificados.
fn discord_messages_from_array(
    arr: Option<&Vec<Value>>,
    channel_id: &str,
    cursor: &Cursor,
) -> Vec<ChatMessage> {
    let mut out: Vec<ChatMessage> = vec![];
    let Some(arr) = arr else { return out };
    for m in arr {
        let id = m.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let ts = m
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(parse_iso8601)
            .unwrap_or(0);
        let author = m
            .get("author")
            .and_then(|a| {
                a.get("global_name")
                    .or_else(|| a.get("username"))
                    .or_else(|| a.get("name"))
            })
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let content = m
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let attachments = m
            .get("attachments")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let kind = if attachments > 0 && content.trim().is_empty() {
            "adjunto".to_string()
        } else {
            "texto".to_string()
        };
        let mid = if id.is_empty() {
            synthetic_message_id(ts, &author, &content)
        } else {
            id
        };
        if !is_newer_than_cursor(ts, &mid, cursor) {
            continue;
        }
        out.push(ChatMessage {
            id: mid,
            ts,
            author,
            text: content,
            kind,
            verified: true,
        });
    }
    let _ = channel_id;
    sort_messages(&mut out);
    out
}

// ========================================================================= //
// 10. Adaptador: Grok (web, como el resto de servicios web)
// ========================================================================= //

/// Endpoints de Grok.
///
/// **NO VERIFICADO**: son los endpoints internos que usa la web de Grok; hay
/// que medirlos en el móvil (ver `measure` en el catálogo).  Si el servicio
/// responde 401/403 se reporta el código TAL CUAL, sin inventar nada.
pub const GROK_ENDPOINTS: &[(&str, &str)] = &[
    ("channels", "https://grok.com/rest/app-chat/conversations"),
    ("messages", "https://grok.com/rest/app-chat/conversations/{id}"),
];

/// Parsea una exportación de Grok (JSON tolerante).
///
/// Acepta `{"conversations":[...]}` o un array de conversaciones con
/// `messages[{role|author, content|text, timestamp|created_at}]`.
pub fn parse_grok_export(text: &str, cursor: &Cursor, limit: usize) -> AdapterReport {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return AdapterReport::error("grok", "el JSON de la exportación no se puede leer");
    };
    let convs = v
        .get("conversations")
        .and_then(|c| c.as_array())
        .cloned()
        .or_else(|| v.as_array().cloned())
        .unwrap_or_default();
    let mut out: Vec<Conversation> = vec![];
    let mut total = 0usize;
    for c in convs.iter().take(limit.clamp(1, 5000)) {
        let id = c
            .get("id")
            .or_else(|| c.get("conversation_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let title = c
            .get("title")
            .or_else(|| c.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut msgs: Vec<ChatMessage> = vec![];
        if let Some(arr) = c.get("messages").and_then(|m| m.as_array()) {
            for m in arr {
                let author = m
                    .get("role")
                    .or_else(|| m.get("author"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("user")
                    .to_string();
                let body = m
                    .get("content")
                    .or_else(|| m.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if body.trim().is_empty() {
                    continue;
                }
                let ts = m
                    .get("timestamp")
                    .or_else(|| m.get("created_at"))
                    .and_then(|v| v.as_str())
                    .and_then(parse_iso8601)
                    .unwrap_or(0);
                let mid = m
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| synthetic_message_id(ts, &author, &body));
                if !is_newer_than_cursor(ts, &mid, cursor) {
                    continue;
                }
                msgs.push(ChatMessage {
                    id: mid,
                    ts,
                    author,
                    text: body,
                    kind: "texto".to_string(),
                    verified: true,
                });
            }
        }
        if msgs.is_empty() {
            continue;
        }
        sort_messages(&mut msgs);
        total += msgs.len();
        let updated_at = msgs.iter().map(|m| m.ts).max().unwrap_or(0);
        out.push(Conversation {
            service: "grok".to_string(),
            conversation_id: if id.is_empty() {
                format!("grok-{}", short_hash(&title))
            } else {
                id
            },
            title: if title.is_empty() {
                "conversación de Grok".to_string()
            } else {
                title
            },
            messages: msgs,
            origin: Origin::Export,
            access: AccessLevel::Completo,
            updated_at,
            source: "export:grok".to_string(),
            note: String::new(),
        });
    }
    AdapterReport {
        service: "grok".to_string(),
        origin: Origin::Export,
        access: if total > 0 {
            AccessLevel::Completo
        } else {
            AccessLevel::Metadatos
        },
        status: if total > 0 { "ok" } else { "parcial" }.to_string(),
        conversations: out,
        schema: json!({ "formato": "conversations[].messages[]" }),
        notes: vec![],
        errors: vec![],
        source: "export:grok".to_string(),
        data: Value::Null,
    }
}

// ========================================================================= //
// 11. Conversión desde el motor de IA existente (`crate::sync`)
// ========================================================================= //

/// Convierte una conversación del esquema de `sync.rs` al modelo unificado.
pub fn from_sync_conversation(v: &Value, service: &str, origin: Origin) -> Option<Conversation> {
    let id = v.get("id").and_then(|x| x.as_str())?;
    if id.is_empty() {
        return None;
    }
    let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let updated_at = v.get("updated_at").and_then(|x| x.as_f64()).unwrap_or(0.0) as i64;
    let source = v.get("source").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut msgs: Vec<ChatMessage> = vec![];
    if let Some(arr) = v.get("messages").and_then(|m| m.as_array()) {
        for (i, m) in arr.iter().enumerate() {
            let text = m
                .get("content")
                .or_else(|| m.get("text"))
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let author = m
                .get("role")
                .or_else(|| m.get("author"))
                .and_then(|x| x.as_str())
                .unwrap_or("user")
                .to_string();
            let ts = m.get("ts").and_then(|x| x.as_f64()).unwrap_or(0.0) as i64;
            if text.trim().is_empty() {
                continue;
            }
            let mid = m
                .get("id")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}-{}", i, short_hash(&text)));
            msgs.push(ChatMessage {
                id: mid,
                ts,
                author,
                text,
                kind: "texto".to_string(),
                verified: true,
            });
        }
    }
    let access = if msgs.is_empty() {
        AccessLevel::Metadatos
    } else {
        AccessLevel::Completo
    };
    Some(Conversation {
        service: service.to_string(),
        conversation_id: id.to_string(),
        title,
        messages: msgs,
        origin,
        access,
        updated_at,
        source,
        note: String::new(),
    })
}

/// Ejecuta el extractor embebido (`assets/sync_extract.py`) en el dispositivo.
///
/// Es el mismo extractor que usa `crate::sync` (NO se duplica): aquí sólo se
/// convierte su salida al modelo unificado.
fn run_extractor(service: &str) -> Result<Vec<Value>, String> {
    let Some(cmd) = crate::sync::extractor_cmd(&[service]) else {
        return Err("no se pudo preparar el extractor embebido".to_string());
    };
    let r = crate::android::root_exec(&cmd, 180_000, 8 * 1024 * 1024);
    if r.timed_out {
        return Err("timeout ejecutando el extractor".to_string());
    }
    let mut out = vec![];
    for line in r.stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("__error__").is_some()
            || v.get("__stats__").is_some()
            || v.get("__cookies__").is_some()
        {
            continue;
        }
        if v.get("service").and_then(|x| x.as_str()) == Some(service) {
            out.push(v);
        }
    }
    Ok(out)
}

// ========================================================================= //
// 12. Capa de dispositivo
// ========================================================================= //

/// Ejecuta una orden como root en el dispositivo.
fn device_shell(cmd: &str, timeout_ms: u64) -> crate::android::ExecResult {
    crate::android::root_exec(cmd, timeout_ms, 4 * 1024 * 1024)
}

/// Lee un fichero del dispositivo (binario) a través de `root_exec`.
///
/// Se lee **por trozos** (`tail -c +N | head -c M | base64`) porque las BDs
/// de las apps de mensajería pueden pesar más que el tope de una sola
/// llamada, y `base64` es lo único que hay en el PATH de `su -mm -c` (no hay
/// python3, ni curl, ni sqlite3).
fn device_read_bytes(path: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    let quoted = crate::android::sh_quote(path);
    let stat = device_shell(&format!("stat -c %s {} 2>/dev/null || echo 0", quoted), 30_000);
    let size: usize = stat.stdout.trim().parse().unwrap_or(0);
    if size == 0 {
        return Err(format!("no se pudo leer {} (fichero vacío o inaccesible)", path));
    }
    const CHUNK: usize = 4 * 1024 * 1024;
    let total = size.min(max_bytes);
    let mut out: Vec<u8> = Vec::with_capacity(total.min(8 * 1024 * 1024));
    let mut offset = 0usize;
    while offset < total {
        let want = CHUNK.min(total - offset);
        let cmd = format!(
            "tail -c +{} {} | head -c {} | base64 -w 0",
            offset + 1,
            quoted,
            want
        );
        let r = device_shell(&cmd, 120_000);
        if r.timed_out {
            return Err(format!("timeout leyendo {}", path));
        }
        let Some(bytes) = decode_base64_bytes(r.stdout.trim()) else {
            return Err(format!("base64 inválido leyendo {}", path));
        };
        if bytes.is_empty() {
            break;
        }
        out.extend_from_slice(&bytes);
        offset += bytes.len();
        if bytes.len() < want {
            break;
        }
    }
    if out.is_empty() {
        return Err(format!("no se pudo leer {}", path));
    }
    Ok(out)
}

/// Decodifica base64 estándar a bytes.
pub fn decode_base64_bytes(s: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut rev = [255u8; 256];
    for (i, c) in TABLE.iter().enumerate() {
        rev[*c as usize] = i as u8;
    }
    let mut out = vec![];
    let mut buf = 0u32;
    let mut bits = 0u32;
    for ch in s.bytes() {
        if ch == b'=' {
            break;
        }
        let v = rev[ch as usize];
        if v == 255 {
            continue;
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// Primer fichero existente entre los candidatos.
fn device_find_first<S: AsRef<str>>(paths: &[S]) -> Option<String> {
    let list: Vec<String> = paths.iter().map(|p| crate::android::sh_quote(p.as_ref())).collect();
    let script = format!(
        "for f in {}; do if [ -e \"$f\" ]; then echo \"$f\"; break; fi; done",
        list.join(" ")
    );
    let r = device_shell(&script, 30_000);
    let found = r.stdout.trim().to_string();
    if found.is_empty() {
        None
    } else {
        Some(found)
    }
}

/// Lista los ficheros de un directorio del dispositivo.
fn device_list_dir(dir: &str) -> Vec<String> {
    let script = format!(
        "for f in {}/*; do [ -e \"$f\" ] || continue; echo \"$f\"; done",
        crate::android::sh_quote(dir)
    );
    let r = device_shell(&script, 30_000);
    r.stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Volcado de notificaciones del sistema.
fn device_notifications() -> Option<String> {
    let r = device_shell("dumpsys notification --noredact", 60_000);
    if r.stdout.trim().is_empty() {
        return None;
    }
    Some(r.stdout)
}

/// Petición HTTP GET con el `curl` del dispositivo (mismo patrón que
/// `sync_web.rs`: Cloudflare rechaza la huella TLS de Rust).
fn device_curl(url: &str, headers: &[(&str, &str)]) -> (u16, String) {
    let Some(curl) = crate::sync::curl_path() else {
        return (0, "no hay curl en el dispositivo".to_string());
    };
    let mut cmd = format!(
        "{} -s --compressed -m 45 -w '\n__MD_HTTP__%{{http_code}}'",
        crate::android::sh_quote(&curl)
    );
    for (k, v) in headers {
        cmd.push_str(&format!(
            " -H '{}: {}'",
            k.replace('\'', "'\''"),
            v.replace('\'', "'\''")
        ));
    }
    cmd.push_str(&format!(" '{}'", url.replace('\'', "'\''")));
    let r = device_shell(&cmd, 60_000);
    if r.timed_out {
        return (0, "timeout".to_string());
    }
    let out = r.stdout;
    match out.rfind("__MD_HTTP__") {
        Some(pos) => {
            let (body, tail) = out.split_at(pos);
            let code: u16 = tail["__MD_HTTP__".len()..].trim().parse().unwrap_or(0);
            (code, body.trim_end().to_string())
        }
        None => (0, out.trim().to_string()),
    }
}

// ========================================================================= //
// 13. Pull por servicio (capa de dispositivo)
// ========================================================================= //

const TELEGRAM_DB_DIR: &str = "/data/data/org.telegram.messenger/databases";
const WHATSAPP_DB_DIR: &str = "/data/data/com.whatsapp/databases";
const WHATSAPP_W4B_DB_DIR: &str = "/data/data/com.whatsapp.w4b/databases";
const SIGNAL_DB_DIR: &str = "/data/data/org.thoughtcrime.securesms/app_database";
const GROK_PACKAGE: &str = "ai.x.grok";
const UA: &str = "Mozilla/5.0 (Linux; Android 14; SM-G998B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";

/// Pull de Telegram (BD local sin cifrar).
pub fn pull_telegram(cursor: &Cursor, limit: usize) -> AdapterReport {
    let files = device_list_dir(TELEGRAM_DB_DIR);
    if files.is_empty() {
        return AdapterReport::sin_acceso(
            "telegram",
            Origin::LocalDb,
            "no se encuentra la base de datos de Telegram en el dispositivo \
             (o no hay root): sin acceso",
        );
    }
    let mut merged = AdapterReport {
        service: "telegram".to_string(),
        origin: Origin::LocalDb,
        access: AccessLevel::Metadatos,
        status: "ok".to_string(),
        ..Default::default()
    };
    for f in files.iter().take(6) {
        if !f.ends_with(".db") {
            continue;
        }
        let Ok(bytes) = device_read_bytes(f, 64 * 1024 * 1024) else {
            merged.errors.push(format!("no se pudo leer {}", f));
            continue;
        };
        let sub = telegram_from_bytes(&bytes, cursor, limit);
        merged.conversations.extend(sub.conversations);
        merged.errors.extend(sub.errors);
        merged.schema = sub.schema.clone();
        if !merged.source.is_empty() {
            merged.source.push_str(", ");
        }
        merged.source.push_str(f);
    }
    if merged.conversations.is_empty() && merged.errors.is_empty() {
        merged.status = "sin_acceso".to_string();
        merged.notes.push("no hay mensajes nuevos desde el cursor".to_string());
    }
    merged.access = if merged.messages() > 0 {
        AccessLevel::Completo
    } else {
        merged.access
    };
    merged
}

/// Pull de WhatsApp: metadatos (`wa.db`) + notificaciones.
pub fn pull_whatsapp(cursor: &Cursor, limit: usize) -> AdapterReport {
    let mut merged = AdapterReport {
        service: "whatsapp".to_string(),
        origin: Origin::LocalDb,
        access: AccessLevel::Metadatos,
        status: "ok".to_string(),
        ..Default::default()
    };

    // 1) wa.db (metadatos, sin cifrar).
    let wa_candidates = [
        format!("{}/wa.db", WHATSAPP_DB_DIR),
        format!("{}/wa.db", WHATSAPP_W4B_DB_DIR),
    ];
    if let Some(wa) = device_find_first(&wa_candidates) {
        if let Ok(bytes) = device_read_bytes(&wa, 32 * 1024 * 1024) {
            let sub = whatsapp_metadata_from_bytes(&bytes, limit);
            merged.conversations.extend(sub.conversations);
            merged.schema = sub.schema.clone();
            merged.data = sub.data.clone();
            merged.notes.extend(sub.notes);
        } else {
            merged.errors.push(format!("no se pudo leer {}", wa));
        }
    }

    // 2) msgstore.db: cifrado (SQLCipher) → estado honesto, sin mensajes.
    let msg_candidates = [
        format!("{}/msgstore.db", WHATSAPP_DB_DIR),
        format!("{}/msgstore.db.crypt15", WHATSAPP_DB_DIR),
        format!("{}/msgstore.db", WHATSAPP_W4B_DB_DIR),
        format!("{}/msgstore.db.crypt15", WHATSAPP_W4B_DB_DIR),
    ];
    if let Some(msg) = device_find_first(&msg_candidates) {
        match device_read_bytes(&msg, 4 * 1024 * 1024) {
            Ok(bytes) => match classify_db(&bytes) {
                DbState::Encrypted => {
                    merged
                        .notes
                        .push(format!("{}: {}", msg, ENCRYPTION_REASON));
                    merged.schema = json!({
                        "msgstore": msg,
                        "cifrado": true,
                        "cabecera": "no-SQLite",
                    });
                }
                DbState::Plain => {
                    merged.notes.push(format!(
                        "{}: la cabecera es SQLite pero el esquema de WhatsApp \
                         no está mapeado: no se inventa ningún mensaje",
                        msg
                    ));
                }
                DbState::Empty => {}
            },
            Err(e) => merged.errors.push(e),
        }
    }

    // 3) Notificaciones: texto realmente visible.
    if let Some(dump) = device_notifications() {
        let convs = notifications_conversations("whatsapp", "com.whatsapp", &dump, cursor);
        merged.conversations.extend(convs);
        merged.source = "dumpsys notification --noredact".to_string();
    } else {
        merged
            .notes
            .push("no se pudo leer 'dumpsys notification'".to_string());
    }

    if merged.conversations.is_empty() && merged.errors.is_empty() {
        merged.status = "sin_acceso".to_string();
    }
    merged
}

/// Pull de Signal: cifrado + notificaciones (metadatos).
pub fn pull_signal(cursor: &Cursor, _limit: usize) -> AdapterReport {
    let mut merged = AdapterReport {
        service: "signal".to_string(),
        origin: Origin::Notifications,
        access: AccessLevel::CifradoNoDescifrado,
        status: "cifrado".to_string(),
        ..Default::default()
    };

    // 1) El almacén: cifrado (se comprueba la cabecera, no se descifra).
    let files = device_list_dir(SIGNAL_DB_DIR);
    for f in files.iter().take(4) {
        if !f.ends_with(".db") {
            continue;
        }
        match device_read_bytes(f, 4 * 1024 * 1024) {
            Ok(bytes) => {
                let sub = signal_db_state(&bytes, f);
                merged.notes.extend(sub.notes);
                merged.schema = sub.schema.clone();
                merged.source = f.clone();
            }
            Err(e) => merged.errors.push(e),
        }
    }
    if files.is_empty() {
        merged
            .notes
            .push("no se encuentra la base de datos de Signal".to_string());
    }

    // 2) Notificaciones: la lista de conversaciones sí es legible.
    if let Some(dump) = device_notifications() {
        let convs =
            notifications_conversations("signal", "org.thoughtcrime.securesms", &dump, cursor);
        if !convs.is_empty() {
            merged.conversations.extend(convs);
            merged.access = AccessLevel::Metadatos;
            merged.status = "parcial".to_string();
            merged.source = "dumpsys notification --noredact".to_string();
        }
    }

    if merged.conversations.is_empty() && merged.errors.is_empty() && merged.notes.is_empty() {
        merged.status = "sin_acceso".to_string();
    }
    merged
}

/// Pull de Discord por API (token del usuario) o por exportación.
pub fn pull_discord(cursor: &Cursor, limit: usize) -> AdapterReport {
    let mut merged = AdapterReport {
        service: "discord".to_string(),
        origin: Origin::WebApi,
        access: AccessLevel::Metadatos,
        status: "ok".to_string(),
        ..Default::default()
    };

    let Some(token) = read_discord_token() else {
        merged.status = "sin_acceso".to_string();
        merged.notes.push(
            "no hay token de Discord: hay que escribirlo una vez en el \
             fichero 0600 del state_dir (`md-sync-chats token --set ...`) o \
             usar la exportación oficial (messages.json)"
                .to_string(),
        );
        return merged;
    };

    let (status, body) = device_curl(
        &discord_channels_url(),
        &[("Authorization", token.as_str()), ("User-Agent", UA)],
    );
    if status != 200 {
        merged.status = "sin_acceso".to_string();
        merged.errors.push(format!("HTTP {} en la API de Discord", status));
        merged.notes.push(
            "el token puede estar caducado o ser inválido: se reporta el \
             código HTTP tal cual, sin inventar conversaciones"
                .to_string(),
        );
        return merged;
    }
    let Ok(v) = serde_json::from_str::<Value>(&body) else {
        merged.status = "error".to_string();
        merged.errors.push("la respuesta de la API no es JSON".to_string());
        return merged;
    };
    let channels = v.as_array().cloned().unwrap_or_default();
    merged.schema = json!({ "channels": channels.len() });
    for ch in channels.iter().take(limit.clamp(1, 200)) {
        let id = ch.get("id").and_then(|x| x.as_str()).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let name = ch
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let (st, body2) = device_curl(
            &discord_messages_url(id, &cursor.last_id),
            &[("Authorization", token.as_str()), ("User-Agent", UA)],
        );
        if st != 200 {
            merged.errors.push(format!("HTTP {} en el canal {}", st, id));
            continue;
        }
        let sub = parse_discord_api_messages(&body2, id, &name, cursor);
        merged.conversations.extend(sub.conversations);
    }
    if merged.conversations.is_empty() && merged.errors.is_empty() {
        merged.status = "parcial".to_string();
        merged
            .notes
            .push("no hay mensajes nuevos desde el cursor".to_string());
    }
    merged
}

/// Pull de Grok por web (mismo patrón que `sync_web.rs`).
pub fn pull_grok(cursor: &Cursor, limit: usize) -> AdapterReport {
    let mut merged = AdapterReport {
        service: "grok".to_string(),
        origin: Origin::WebApi,
        access: AccessLevel::Metadatos,
        status: "ok".to_string(),
        ..Default::default()
    };

    let Some(cookie) = grok_session_cookie() else {
        merged.status = "sin_acceso".to_string();
        merged.notes.push(
            "no hay sesión de Grok: las cookies de Chrome están cifradas por \
             el AndroidKeyStore (no exportables ni con root). Hace falta el \
             hook de Frida (`assets/frida_chrome_cookies.js`) o la exportación \
             oficial de Grok"
                .to_string(),
        );
        return merged;
    };

    // NO VERIFICADO: endpoint interno de grok.com (ver catálogo).
    let (status, body) = device_curl(
        GROK_ENDPOINTS[0].1,
        &[("Cookie", cookie.as_str()), ("User-Agent", UA)],
    );
    if status != 200 {
        merged.status = "sin_acceso".to_string();
        merged.errors.push(format!("HTTP {} en {}", status, GROK_ENDPOINTS[0].1));
        merged.notes.push(
            "endpoint NO VERIFICADO: hay que medirlo en el móvil (ver las \
             órdenes de `measure` del catálogo)"
                .to_string(),
        );
        return merged;
    }
    let Ok(v) = serde_json::from_str::<Value>(&body) else {
        merged.status = "error".to_string();
        merged.errors.push("la respuesta de Grok no es JSON".to_string());
        return merged;
    };
    let convs = v
        .get("conversations")
        .and_then(|c| c.as_array())
        .cloned()
        .or_else(|| v.as_array().cloned())
        .unwrap_or_default();
    merged.schema = json!({ "conversations": convs.len() });
    let mut total = 0usize;
    for c in convs.iter().take(limit.clamp(1, 500)) {
        let id = c
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let title = c
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let mut msgs: Vec<ChatMessage> = vec![];
        if let Some(arr) = c.get("messages").and_then(|m| m.as_array()) {
            for m in arr {
                let body = m
                    .get("content")
                    .or_else(|| m.get("text"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if body.trim().is_empty() {
                    continue;
                }
                let author = m
                    .get("role")
                    .or_else(|| m.get("author"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("user")
                    .to_string();
                let ts = m
                    .get("timestamp")
                    .and_then(|x| x.as_f64())
                    .unwrap_or(0.0) as i64;
                let mid = m
                    .get("id")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| synthetic_message_id(ts, &author, &body));
                if !is_newer_than_cursor(ts, &mid, cursor) {
                    continue;
                }
                msgs.push(ChatMessage {
                    id: mid,
                    ts,
                    author,
                    text: body,
                    kind: "texto".to_string(),
                    verified: true,
                });
            }
        }
        if msgs.is_empty() {
            continue;
        }
        sort_messages(&mut msgs);
        total += msgs.len();
        merged.conversations.push(Conversation {
            service: "grok".to_string(),
            conversation_id: id,
            title,
            messages: msgs,
            origin: Origin::WebApi,
            access: AccessLevel::Completo,
            updated_at: 0,
            source: GROK_ENDPOINTS[0].1.to_string(),
            note: String::new(),
        });
    }
    if total > 0 {
        merged.access = AccessLevel::Completo;
    } else {
        merged.status = "parcial".to_string();
    }
    merged
}

/// Cookies en claro de la WebView de la app de Grok.
///
/// Es el mismo criterio que `sync_web.rs`: sólo valen las cookies **en
/// claro**; las cifradas por el AndroidKeyStore necesitan el hook de Frida.
fn grok_session_cookie() -> Option<String> {
    let db_path = format!("/{}/app_webview/Default/Cookies", GROK_PACKAGE);
    let bytes = device_read_bytes(&db_path, 16 * 1024 * 1024).ok()?;
    let path = temp_db_file(&bytes, "grok_cookies").ok()?;
    let out = chromium_cookies(&path);
    let _ = fs::remove_file(&path);
    if out.is_empty() {
        None
    } else {
        Some(out.join("; "))
    }
}

/// Extrae las cookies EN CLARO de una BD de Chromium.
pub fn chromium_cookies(path: &Path) -> Vec<String> {
    let Ok(conn) = open_ro(path) else {
        return vec![];
    };
    let sql = "SELECT name, value FROM cookies WHERE encrypted_value IS NULL OR length(encrypted_value) = 0";
    let Ok(mut st) = conn.prepare(sql) else {
        return vec![];
    };
    let Ok(mut rows) = st.query_map([], |row| {
        let n = cell_text(row, 0).unwrap_or_default();
        let v = cell_text(row, 1).unwrap_or_default();
        Ok::<String, rusqlite::Error>(format!("{}={}", n, v))
    }) else {
        return vec![];
    };
    let mut out = vec![];
    while let Some(Ok(c)) = rows.next() {
        if c.len() > 2 {
            out.push(c);
        }
    }
    out
}

/// Pull de un servicio de IA por su motor local (`crate::sync`).
fn pull_ai_local(service: &str, cursor: &Cursor) -> AdapterReport {
    match run_extractor(service) {
        Ok(items) => {
            let mut convs: Vec<Conversation> = vec![];
            for v in items {
                if let Some(c) = from_sync_conversation(&v, service, Origin::LocalDb) {
                    // Incremental: sólo lo nuevo desde el cursor.
                    let fresh: Vec<ChatMessage> = c
                        .messages
                        .iter()
                        .filter(|m| is_newer_than_cursor(m.ts, &m.id, cursor))
                        .cloned()
                        .collect();
                    if fresh.is_empty() {
                        continue;
                    }
                    let mut c2 = c.clone();
                    c2.updated_at = fresh.iter().map(|m| m.ts).max().unwrap_or(0);
                    c2.messages = fresh;
                    convs.push(c2);
                }
            }
            let n = convs.iter().map(|c| c.verified_messages()).sum::<usize>();
            AdapterReport {
                service: service.to_string(),
                origin: Origin::LocalDb,
                access: if n > 0 {
                    AccessLevel::Completo
                } else {
                    AccessLevel::Metadatos
                },
                status: if n > 0 { "ok" } else { "parcial" }.to_string(),
                conversations: convs,
                schema: json!({ "motor": "crate::sync::run_sync" }),
                notes: vec![],
                errors: vec![],
                source: "sync_extract.py".to_string(),
    data: Value::Null,
            }
        }
        Err(e) => AdapterReport::sin_acceso(service, Origin::LocalDb, &e),
    }
}

/// Pull de un servicio de IA por su motor web (`crate::sync_web`).
fn pull_ai_web(service: &str, cursor: &Cursor) -> AdapterReport {
    let res = crate::sync_web::web_pull(Some(vec![service.to_string()]));
    let errors: Vec<String> = res
        .get("errors")
        .and_then(|e| e.as_array())
        .map(|arr| {
            arr.iter()
                .map(|e| {
                    format!(
                        "{}: {}",
                        e.get("service").and_then(|x| x.as_str()).unwrap_or("?"),
                        e.get("error").and_then(|x| x.as_str()).unwrap_or("?")
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    // `web_pull` guarda en el almacén de `sync.rs`; aquí se lee de vuelta para
    // convertirlo al modelo unificado (así NO se duplica el extractor).
    let mut convs: Vec<Conversation> = vec![];
    let listing = crate::sync::list_conversations(Some(service), 1000);
    if let Some(items) = listing.get("conversations").and_then(|c| c.as_array()) {
        for it in items {
            let id = it.get("id").and_then(|x| x.as_str()).unwrap_or("");
            if id.is_empty() {
                continue;
            }
            let full = crate::sync::get_conversation(service, id);
            if let Some(c) = full.get("conversation") {
                if let Some(c) = from_sync_conversation(c, service, Origin::WebApi) {
                    let fresh: Vec<ChatMessage> = c
                        .messages
                        .iter()
                        .filter(|m| is_newer_than_cursor(m.ts, &m.id, cursor))
                        .cloned()
                        .collect();
                    if fresh.is_empty() {
                        continue;
                    }
                    let mut c2 = c.clone();
                    c2.updated_at = fresh.iter().map(|m| m.ts).max().unwrap_or(0);
                    c2.messages = fresh;
                    convs.push(c2);
                }
            }
        }
    }
    let n = convs.iter().map(|c| c.verified_messages()).sum::<usize>();
    AdapterReport {
        service: service.to_string(),
        origin: Origin::WebApi,
        access: if n > 0 {
            AccessLevel::Completo
        } else {
            AccessLevel::Metadatos
        },
        status: if n > 0 {
            "ok".to_string()
        } else if errors.is_empty() {
            "parcial".to_string()
        } else {
            "sin_acceso".to_string()
        },
        conversations: convs,
        schema: json!({ "motor": "crate::sync_web::web_pull" }),
        notes: vec![],
        errors,
        source: "sync_web.rs".to_string(),
        data: Value::Null,
    }
}

// ========================================================================= //
// 14. API de alto nivel
// ========================================================================= //

/// Lista el catálogo de servicios (para `list` y para el MCP).
pub fn list_services() -> Value {
    let items: Vec<Value> = catalog()
        .iter()
        .map(|s| {
            json!({
                "service": s.service,
                "label": s.label,
                "kind": format!("{:?}", s.kind),
                "origin": s.origin.as_str(),
                "access": s.access.as_str(),
                "data_source": s.data_source,
                "schema": s.schema,
                "route": s.route,
                "missing": s.missing,
                "measure": s.measure,
            })
        })
        .collect();
    json!({ "ok": true, "services": items, "count": items.len() })
}

/// Ejecuta el pull de un servicio y fusiona el resultado en el almacén.
pub fn pull(service: &str) -> Value {
    let Some(spec) = spec(service) else {
        return json!({
            "ok": false,
            "error": "servicio_desconocido",
            "hint": "usa `list` para ver los servicios",
            "services": service_names(),
        });
    };
    let cursor = cursor_for(service);
    let limit = 5000usize;
    let report = match spec.kind {
        ServiceKind::LocalDb => match service {
            "telegram" => pull_telegram(&cursor, limit),
            "whatsapp" | "whatsapp_business" => pull_whatsapp(&cursor, limit),
            _ => AdapterReport::sin_acceso(service, spec.origin, "sin adaptador local"),
        },
        ServiceKind::Notifications => match service {
            "signal" => pull_signal(&cursor, limit),
            _ => AdapterReport::sin_acceso(service, spec.origin, "sin adaptador de notificaciones"),
        },
        ServiceKind::WebApi => match service {
            "discord" => pull_discord(&cursor, limit),
            "grok" => pull_grok(&cursor, limit),
            _ => AdapterReport::sin_acceso(service, spec.origin, "sin adaptador web"),
        },
        ServiceKind::Export => AdapterReport::sin_acceso(
            service,
            spec.origin,
            "la exportación se ingesta con `md-sync-chats ingest <fichero>`",
        ),
        ServiceKind::AiLocal => pull_ai_local(service, &cursor),
        ServiceKind::AiWeb => pull_ai_web(service, &cursor),
    };

    // Merge idempotente por `servicio::conversation_id::message_id`.
    let mut store = load_store();
    let stats = merge_all(&mut store, report.conversations.clone());
    let _ = save_store(&store);

    // Cursor: se avanza con lo realmente consumido.
    let all_msgs: Vec<ChatMessage> = report
        .conversations
        .iter()
        .flat_map(|c| c.messages.clone())
        .collect();
    let next = advance_cursor(service, &cursor, &all_msgs);
    let _ = save_cursor(&next);

    json!({
        "ok": report.status != "error",
        "service": service,
        "status": report.status,
        "origin": report.origin.as_str(),
        "access": report.access.as_str(),
        "conversations": report.conversations.len(),
        "messages": report.messages(),
        "added": stats.added,
        "updated": stats.updated,
        "new_messages": stats.new_messages,
        "repeated_messages": stats.repeated_messages,
        "cursor": {
            "last_ts": next.last_ts,
            "last_id": next.last_id,
            "updated_at": next.updated_at,
        },
        "schema": report.schema,
        "notes": report.notes,
        "errors": report.errors,
        "items": report.conversations,
    })
}

/// Ingesta una exportación oficial (Discord, Grok, ChatGPT, ...).
///
/// La detección del formato es la misma que usa `sync_web.rs`; aquí se
/// añade el modelo unificado (con `origen` y `nivel`).
///
/// El fichero puede estar en el sistema donde corre el binario **o** en el
/// dispositivo (si no está local, se lee por `root_exec`).
pub fn ingest_export(path: &str) -> Value {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => match read_device_file(path) {
            Some(t) => t,
            None => {
                return json!({
                    "ok": false,
                    "error": "no_se_puede_leer",
                    "hint": format!("{}: {}", path, e),
                })
            }
        },
    };
    let head: String = text.chars().take(4000).collect();
    let report = if head.contains("channels") && head.contains("messages") {
        parse_discord_export(&text, &Cursor::default(), 5000)
    } else if head.contains("conversations") || head.contains("\"messages\"") {
        parse_grok_export(&text, &Cursor::default(), 5000)
    } else if let Some(fmt) = crate::sync_web::detect_export_format("conversations.json", &head) {
        let convs = crate::sync_web::parse_export(&text, fmt);
        let mut out = vec![];
        for c in convs {
            let svc = c.get("service").and_then(|x| x.as_str()).unwrap_or("").to_string();
            if let Some(u) = from_sync_conversation(&c, &svc, Origin::Export) {
                out.push(u);
            }
        }
        AdapterReport {
            service: svc_name(&out),
            origin: Origin::Export,
            access: AccessLevel::Completo,
            status: "ok".to_string(),
            conversations: out,
            schema: json!({ "formato": fmt }),
            notes: vec![],
            errors: vec![],
            source: format!("export:{}", path),
    data: Value::Null,
        }
    } else {
        return json!({
            "ok": false,
            "error": "formato_desconocido",
            "hint": "no parece una exportación de Discord, Grok, ChatGPT ni Takeout",
        });
    };

    let mut store = load_store();
    let stats = merge_all(&mut store, report.conversations.clone());
    let _ = save_store(&store);
    json!({
        "ok": true,
        "path": path,
        "origin": report.origin.as_str(),
        "access": report.access.as_str(),
        "conversations": report.conversations.len(),
        "messages": report.messages(),
        "added": stats.added,
        "updated": stats.updated,
        "new_messages": stats.new_messages,
    })
}

fn svc_name(convs: &[Conversation]) -> String {
    convs
        .first()
        .map(|c| c.service.clone())
        .unwrap_or_else(|| "desconocido".to_string())
}

/// Lee un fichero de texto del dispositivo (si no está en local).
fn read_device_file(path: &str) -> Option<String> {
    if !path.starts_with('/') || path.contains("..") {
        return None;
    }
    let cmd = format!(
        "head -c 8000000 {} | base64 -w 0",
        crate::android::sh_quote(path)
    );
    let r = device_shell(&cmd, 120_000);
    if r.stdout.trim().is_empty() {
        return None;
    }
    let bytes = decode_base64_bytes(r.stdout.trim())?;
    String::from_utf8(bytes).ok()
}

/// Estado del almacén de chats.
pub fn status() -> Value {
    let store = load_store();
    let cursors = load_cursors();
    let mut per_service: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    for c in store.values() {
        let e = per_service
            .entry(c.service.clone())
            .or_insert((0, 0, 0));
        e.0 += 1;
        e.1 += c.verified_messages();
        e.2 += c.messages.len();
    }
    let mut services = serde_json::Map::new();
    let mut total_c = 0usize;
    let mut total_m = 0usize;
    for (svc, (c, m, raw)) in &per_service {
        services.insert(
            svc.clone(),
            json!({
                "conversations": c,
                "messages": m,
                "messages_raw": raw,
            }),
        );
        total_c += c;
        total_m += m;
    }
    let mut cats = serde_json::Map::new();
    for s in catalog() {
        let cursor = cursors.get(s.service).cloned();
        cats.insert(
            s.service.to_string(),
            json!({
                "label": s.label,
                "origin": s.origin.as_str(),
                "access": s.access.as_str(),
                "missing": s.missing,
                "cursor": cursor,
                "in_store": per_service.get(s.service).map(|e| e.0).unwrap_or(0),
            }),
        );
    }
    json!({
        "ok": true,
        "store": store_dir().to_string_lossy(),
        "services": Value::Object(services),
        "catalog": Value::Object(cats),
        "total_conversations": total_c,
        "total_messages": total_m,
        "discord_token": if read_discord_token().is_some() { "presente" } else { "ausente" },
    })
}

/// Lista las conversaciones del almacén (con su origen y su nivel).
pub fn list_conversations(service: Option<&str>, limit: usize) -> Value {
    let store = load_store();
    let mut items: Vec<Value> = store
        .values()
        .filter(|c| match service {
            Some(s) => c.service == s,
            None => true,
        })
        .map(|c| {
            json!({
                "service": c.service,
                "conversation_id": c.conversation_id,
                "title": c.title,
                "messages": c.verified_messages(),
                "origin": c.origin.as_str(),
                "access": c.access.as_str(),
                "updated_at": c.updated_at,
                "source": c.source,
                "note": c.note,
            })
        })
        .collect();
    items.sort_by(|a, b| {
        let ta = a["updated_at"].as_i64().unwrap_or(0);
        let tb = b["updated_at"].as_i64().unwrap_or(0);
        tb.cmp(&ta)
    });
    let total = items.len();
    items.truncate(limit.clamp(1, 1000));
    json!({
        "ok": true,
        "total": total,
        "returned": items.len(),
        "conversations": items,
    })
}

/// Devuelve una conversación completa del almacén.
pub fn get_conversation(service: &str, conversation_id: &str) -> Value {
    let store = load_store();
    match store.get(&conversation_key(service, conversation_id)) {
        Some(c) => json!({ "ok": true, "conversation": c }),
        None => json!({
            "ok": false,
            "error": "no_encontrada",
            "hint": "no existe esa conversación en el almacén de chats",
        }),
    }
}

/// Migra las conversaciones del almacén de `sync.rs` al modelo unificado.
///
/// Sirve para lo que ya se sincronizó antes de existir este módulo (las apps
/// de IA) sin volver a descargarlo del dispositivo.
pub fn migrate_legacy() -> Value {
    let mut out: Vec<Conversation> = vec![];
    for service in crate::sync::SERVICES {
        let listing = crate::sync::list_conversations(Some(service), 1000);
        if let Some(items) = listing.get("conversations").and_then(|c| c.as_array()) {
            for it in items {
                let id = it.get("id").and_then(|x| x.as_str()).unwrap_or("");
                if id.is_empty() {
                    continue;
                }
                let full = crate::sync::get_conversation(service, id);
                if let Some(c) = full.get("conversation") {
                    if let Some(u) = from_sync_conversation(c, service, Origin::LocalDb) {
                        out.push(u);
                    }
                }
            }
        }
    }
    let migrated = out.len();
    let mut store = load_store();
    let stats = merge_all(&mut store, out);
    let _ = save_store(&store);
    json!({
        "ok": true,
        "migrated": migrated,
        "added": stats.added,
        "updated": stats.updated,
        "new_messages": stats.new_messages,
    })
}

// ========================================================================= //
// 15. Definiciones MCP
// ========================================================================= //

/// Herramientas MCP del motor de chats.
///
/// La web y el MCP reciben SIEMPRE `origin` y `access` en cada
/// conversación, para poder distinguir un mensaje real de una notificación o
/// de un hueco cifrado.
pub fn tool_definitions() -> Value {
    let services: Vec<&str> = service_names();
    json!([
        {
            "name": "sync_chats_pull",
            "description": "Sincroniza las conversaciones de un servicio (WhatsApp, Telegram, Signal, Discord, Grok, ...) con su origen real (local_db/web_api/export/notificaciones) y su nivel de acceso (completo/metadatos/cifrado_no_descifrado). Incremental: sólo descarga lo nuevo desde el cursor del servicio. Los almacenes cifrados se reportan como `cifrado_no_descifrado` SIN inventar mensajes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "service": { "type": "string", "enum": services },
                    "limit": { "type": "integer", "default": 5000, "maximum": 20000 }
                },
                "required": ["service"]
            }
        },
        {
            "name": "sync_chats_status",
            "description": "Estado del almacén de chats: conversaciones y mensajes por servicio, cursor incremental de cada servicio y si el token de Discord está presente.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "sync_chats_list",
            "description": "Lista las conversaciones sincronizadas con su origen (local_db/web_api/export/notificaciones), su nivel de acceso y el número de mensajes verificados.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "service": { "type": "string" },
                    "limit": { "type": "integer", "default": 50, "maximum": 1000 }
                }
            }
        },
        {
            "name": "sync_chats_catalog",
            "description": "Catálogo de servicios: de dónde están los datos, el esquema real medido, la vía de acceso, el nivel máximo y qué falta para llegar al nivel completo.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "sync_chats_conversation",
            "description": "Devuelve una conversación completa (servicio + conversation_id) con sus mensajes, su origen y su nivel de acceso.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "service": { "type": "string" },
                    "conversation_id": { "type": "string" }
                },
                "required": ["service", "conversation_id"]
            }
        },
        {
            "name": "sync_chats_ingest_export",
            "description": "Ingesta una exportación oficial (Discord messages.json, Grok, ChatGPT conversations.json, Google Takeout) en el almacén de chats.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        }
    ])
}

/// Despacha las herramientas de chats.
pub fn dispatch(name: &str, args: &Value) -> Option<Value> {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let n = |k: &str| args.get(k).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    match name {
        "sync_chats_pull" => {
            let svc = s("service");
            if svc.is_empty() {
                return Some(json!({ "ok": false, "error": "falta_service" }));
            }
            Some(pull(&svc))
        }
        "sync_chats_status" => Some(status()),
        "sync_chats_list" => {
            let svc = s("service");
            Some(list_conversations(
                if svc.is_empty() { None } else { Some(svc.as_str()) },
                if n("limit") == 0 { 50 } else { n("limit") },
            ))
        }
        "sync_chats_catalog" => Some(list_services()),
        "sync_chats_conversation" => Some(get_conversation(&s("service"), &s("conversation_id"))),
        "sync_chats_ingest_export" => Some(ingest_export(&s("path"))),
        _ => None,
    }
}

// ========================================================================= //
// 16. Documentación generada desde el catálogo
// ========================================================================= //

/// Genera la tabla de la documentación (`docs/SYNC_CHATS_POR_SERVICIO.md`).
///
/// Se genera desde el catálogo para que la doc y el código no se separen.
pub fn catalog_markdown() -> String {
    let mut out = String::new();
    out.push_str("| Servicio | Dónde están los datos | Esquema real medido | Vía de acceso | Nivel | Qué falta |\n");
    out.push_str("|---|---|---|---|---|---|\n");
    for s in catalog() {
        out.push_str(&format!(
            "| **{}** (`{}`) | {} | {} | {} | `{}` | {} |\n",
            s.label,
            s.service,
            s.schema.replace('\n', " ").replace("|", "\\|"),
            s.data_source.replace('\n', " ").replace("|", "\\|"),
            s.route.replace('\n', " ").replace("|", "\\|"),
            s.access.as_str(),
            s.missing.replace('\n', " ").replace("|", "\\|"),
        ));
    }
    out
}

/// Comandos de medición (por servicio) en formato de bloque de código.
pub fn measure_markdown() -> String {
    let mut out = String::new();
    for s in catalog() {
        out.push_str(&format!("### {}\n\n```bash\n", s.label));
        for m in s.measure {
            out.push_str(m);
            out.push('\n');
        }
        out.push_str("```\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------- fixtures --

    /// Crea una BD de Telegram mínima con el esquema real medido.
    fn fixture_telegram_db(path: &Path) {
        let conn = Connection::open(path).expect("debe abrir la BD de prueba");
        conn.execute_batch(
            "CREATE TABLE messages (
                uid INTEGER PRIMARY KEY,
                chat_id INTEGER,
                from_id INTEGER,
                date INTEGER,
                message TEXT
             );
             CREATE TABLE chats (
                chat_id INTEGER,
                title TEXT
             );
             CREATE TABLE users (
                user_id INTEGER,
                first_name TEXT
             );
             CREATE TABLE info (
                key TEXT,
                value TEXT
             );
             INSERT INTO info VALUES ('user_id', '7');
             INSERT INTO users VALUES (7, 'Yo');
             INSERT INTO users VALUES (9, 'Ana');
             INSERT INTO chats VALUES (42, 'Ana');
             INSERT INTO messages VALUES (1, 42, 9, 1700000000, 'hola');
             INSERT INTO messages VALUES (2, 42, 7, 1700000060, 'hola Ana');
             INSERT INTO messages VALUES (3, 42, 9, 1700000120, '¿que tal?');
            ",
        )
        .expect("debe crear el esquema");
    }

    fn fixture_path(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("md_sync_chats_test_{}_{}.db", tag, std::process::id()));
        p
    }

    /// Crea una `wa.db` mínima (NO va cifrada): contactos + chats.
    fn fixture_whatsapp_db(path: &Path) {
        let conn = Connection::open(path).expect("debe abrir la BD de prueba");
        conn.execute_batch(
            "CREATE TABLE wa_contacts (jid TEXT, display_name TEXT, number TEXT);
             CREATE TABLE wa_chats (jid TEXT, unread_count INTEGER, message_count INTEGER,
                 last_message_time INTEGER);
             INSERT INTO wa_contacts VALUES ('34600111222@s.whatsapp.net', 'Ana', '34600111222');
             INSERT INTO wa_contacts VALUES ('34600999888@s.whatsapp.net', 'Luis', '34600999888');
             INSERT INTO wa_chats VALUES ('34600111222@s.whatsapp.net', 3, 12, 1700000000);",
        )
        .expect("debe crear el esquema");
    }

    /// Volcado de `dumpsys notification` mínimo (formato real).
    const DUMPSYS_FIXTURE: &str = r#"NotificationRecord(0x1234: pkg=com.whatsapp user=android uid=10123)
    extras={
      android.title=String (Ana)
      android.text=String (hola, ¿estás?)
    }
    postTime=11-20 12:34:56.789
    NotificationRecord(0x5678: pkg=org.thoughtcrime.securesms user=android uid=10456)
    extras={
      android.title=String (Luis)
      android.text=String (mañana a las 10)
    }
    postTime=11-20 13:00:00.000
"#;

    const DISCORD_EXPORT_FIXTURE: &str = r#"{
      "channels": [
        {
          "id": "111222333",
          "name": "general",
          "type": "GuildTextChat",
          "guild": { "id": "999", "name": "MegaDaemon" },
          "messages": [
            {"id": "1001", "timestamp": "2024-01-02T03:04:05.000+00:00",
             "content": "hola discord", "author": {"id": "5", "username": "ana"}},
            {"id": "1002", "timestamp": "2024-01-02T03:05:05.000+00:00",
             "content": "hola de nuevo", "author": {"id": "6", "username": "luis"}}
          ]
        }
      ]
    }"#;

    // ------------------------------------------------------------- modelo --

    #[test]
    fn los_nombres_de_origen_y_nivel_son_los_establecidos() {
        assert_eq!(Origin::LocalDb.as_str(), "local_db");
        assert_eq!(Origin::WebApi.as_str(), "web_api");
        assert_eq!(Origin::Export.as_str(), "export");
        assert_eq!(Origin::Notifications.as_str(), "notificaciones");
        assert_eq!(AccessLevel::Completo.as_str(), "completo");
        assert_eq!(AccessLevel::Metadatos.as_str(), "metadatos");
        assert_eq!(
            AccessLevel::CifradoNoDescifrado.as_str(),
            "cifrado_no_descifrado"
        );
        assert_eq!(Origin::parse("notificaciones"), Some(Origin::Notifications));
        assert_eq!(AccessLevel::parse("cifrado_no_descifrado"), Some(AccessLevel::CifradoNoDescifrado));
        assert_eq!(Origin::parse("inventado"), None);
    }

    #[test]
    fn la_clave_de_mensaje_es_servicio_conversacion_mensaje() {
        assert_eq!(
            message_key("telegram", "chat_42", "7"),
            "telegram::chat_42::7"
        );
        assert_ne!(
            message_key("telegram", "chat_42", "7"),
            message_key("whatsapp", "chat_42", "7")
        );
    }

    #[test]
    fn el_nivel_de_acceso_sube_pero_no_baja_al_fusionar() {
        let meta = Conversation {
            service: "signal".into(),
            conversation_id: "c1".into(),
            title: "Luis".into(),
            messages: vec![],
            origin: Origin::Notifications,
            access: AccessLevel::Metadatos,
            updated_at: 0,
            source: String::new(),
            note: String::new(),
        };
        let mut full = meta.clone();
        full.access = AccessLevel::Completo;
        full.origin = Origin::LocalDb;
        let merged = merge_conversation(Some(meta.clone()), full.clone());
        assert_eq!(merged.access, AccessLevel::Completo);
        assert_eq!(merged.origin, Origin::LocalDb);

        // Y al revés: un pull parcial NO degrada una conversación completa.
        let merged2 = merge_conversation(Some(full.clone()), meta.clone());
        assert_eq!(merged2.access, AccessLevel::Completo);
    }

    #[test]
    fn el_merge_es_idempotente_por_mensaje() {
        let conv = Conversation {
            service: "telegram".into(),
            conversation_id: "chat_42".into(),
            title: "Ana".into(),
            messages: vec![ChatMessage {
                id: "1".into(),
                ts: 10,
                author: "peer".into(),
                text: "hola".into(),
                kind: "texto".into(),
                verified: true,
            }],
            origin: Origin::LocalDb,
            access: AccessLevel::Completo,
            updated_at: 10,
            source: String::new(),
            note: String::new(),
        };
        let mut store = BTreeMap::new();
        let s1 = merge_all(&mut store, vec![conv.clone()]);
        assert_eq!(s1.added, 1);
        assert_eq!(s1.new_messages, 1);

        // Segundo merge del MISMO contenido: no se duplica nada.
        let s2 = merge_all(&mut store, vec![conv.clone()]);
        assert_eq!(s2.added, 0);
        assert_eq!(s2.updated, 1);
        assert_eq!(s2.new_messages, 0);
        assert_eq!(s2.repeated_messages, 1);
        assert_eq!(store.len(), 1);
        assert_eq!(
            store
                .get("telegram::chat_42")
                .unwrap()
                .messages
                .len(),
            1
        );
    }

    #[test]
    fn el_cursor_solo_deja_pasar_lo_nuevo() {
        let cursor = Cursor {
            service: "telegram".into(),
            last_ts: 100,
            last_id: "5".into(),
            ..Default::default()
        };
        assert!(is_newer_than_cursor(101, "1", &cursor));
        assert!(!is_newer_than_cursor(100, "5", &cursor));
        assert!(!is_newer_than_cursor(100, "4", &cursor));
        assert!(is_newer_than_cursor(100, "6", &cursor));
        assert!(!is_newer_than_cursor(99, "9", &cursor));

        // Cursor vacío: todo es nuevo.
        let empty = Cursor::default();
        assert!(is_newer_than_cursor(0, "", &empty));
    }

    #[test]
    fn el_cursor_avanza_con_el_mensaje_mas_reciente() {
        let cursor = Cursor::default();
        let msgs = vec![
            ChatMessage {
                id: "1".into(),
                ts: 10,
                ..Default::default()
            },
            ChatMessage {
                id: "2".into(),
                ts: 20,
                ..Default::default()
            },
            ChatMessage {
                id: "3".into(),
                ts: 20,
                ..Default::default()
            },
        ];
        let next = advance_cursor("telegram", &cursor, &msgs);
        assert_eq!(next.last_ts, 20);
        assert_eq!(next.last_id, "3");
    }

    // ------------------------------------------------------- telegram --

    #[test]
    fn telegram_lee_el_fixture_y_mapea_chat_y_mensajes() {
        let path = fixture_path("tg");
        let _ = fs::remove_file(&path);
        fixture_telegram_db(&path);
        let cursor = Cursor::default();
        let rep = telegram_from_path(&path, &cursor, 100);
        assert_eq!(rep.status, "ok");
        assert_eq!(rep.access, AccessLevel::Completo);
        assert_eq!(rep.conversations.len(), 1);
        let c = &rep.conversations[0];
        assert_eq!(c.service, "telegram");
        assert_eq!(c.conversation_id, "chat_42");
        assert_eq!(c.title, "Ana");
        assert_eq!(c.origin, Origin::LocalDb);
        assert_eq!(c.messages.len(), 3);
        // El primero lo escribió el peer (Ana), el segundo el usuario.
        assert_eq!(c.messages[0].author, "Ana");
        assert_eq!(c.messages[0].text, "hola");
        assert_eq!(c.messages[1].author, "user");
        assert_eq!(c.messages[1].ts, 1700000060);
        assert!(c.messages.iter().all(|m| m.verified));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn telegram_es_incremental_con_el_cursor() {
        let path = fixture_path("tg2");
        let _ = fs::remove_file(&path);
        fixture_telegram_db(&path);

        // Primera pasada: todo.
        let rep = telegram_from_path(&path, &Cursor::default(), 100);
        assert_eq!(rep.messages(), 3);

        // Segunda pasada con cursor en el último mensaje: nada nuevo.
        let cursor = Cursor {
            service: "telegram".into(),
            last_ts: 1700000120,
            last_id: "3".into(),
            ..Default::default()
        };
        let rep2 = telegram_from_path(&path, &cursor, 100);
        assert_eq!(rep2.messages(), 0);

        // Cursor intermedio: sólo el último mensaje.
        let cursor2 = Cursor {
            service: "telegram".into(),
            last_ts: 1700000060,
            last_id: "2".into(),
            ..Default::default()
        };
        let rep3 = telegram_from_path(&path, &cursor2, 100);
        assert_eq!(rep3.messages(), 1);
        assert_eq!(rep3.conversations[0].messages[0].text, "¿que tal?");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn una_bd_cifrada_no_devuelve_mensajes_inventados() {
        // 32 bytes que NO son la cabecera de SQLite (= SQLCipher).
        let cifrado = vec![0xABu8; 32];
        assert_eq!(classify_db(&cifrado), DbState::Encrypted);
        let rep = telegram_from_bytes(&cifrado, &Cursor::default(), 10);
        assert_eq!(rep.status, "cifrado");
        assert_eq!(rep.access, AccessLevel::CifradoNoDescifrado);
        assert_eq!(rep.conversations.len(), 0);
        assert_eq!(rep.messages(), 0);
        assert!(!rep.notes.is_empty());
    }

    // ------------------------------------------------------- whatsapp --

    #[test]
    fn whatsapp_detecta_el_almacen_cifrado_y_no_inventa_mensajes() {
        let cifrado = vec![0x5Au8; 64];
        let rep = whatsapp_metadata_from_bytes(&cifrado, 10);
        assert_eq!(rep.access, AccessLevel::CifradoNoDescifrado);
        assert_eq!(rep.conversations.len(), 0);
        assert_eq!(rep.messages(), 0);
    }

    #[test]
    fn whatsapp_lee_los_contactos_reales_y_no_inventa_mensajes() {
        let path = fixture_path("wa");
        let _ = fs::remove_file(&path);
        fixture_whatsapp_db(&path);
        let rep = whatsapp_metadata_from_path(&path, 100);
        assert_eq!(rep.status, "parcial");
        assert_eq!(rep.access, AccessLevel::Metadatos);

        // Contactos REALES (la tabla de contactos de wa.db).
        let contacts = rep.data["contacts"].as_array().unwrap();
        assert_eq!(contacts.len(), 2);
        assert_eq!(contacts[0]["display_name"], json!("Ana"));
        assert_eq!(contacts[0]["jid"], json!("34600111222@s.whatsapp.net"));

        // Los chats son metadatos: sus entradas NO son mensajes verificados.
        assert_eq!(rep.conversations.len(), 1);
        let c = &rep.conversations[0];
        assert_eq!(c.service, "whatsapp");
        assert_eq!(c.access, AccessLevel::Metadatos);
        assert_eq!(c.origin, Origin::LocalDb);
        assert_eq!(c.messages.len(), 1);
        assert!(!c.messages[0].verified, "un contador no leídos no es un mensaje");
        assert_eq!(c.verified_messages(), 0);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn las_notificaciones_son_texto_real_y_de_origen_notificaciones() {
        let convs = notifications_conversations(
            "whatsapp",
            "com.whatsapp",
            DUMPSYS_FIXTURE,
            &Cursor::default(),
        );
        assert_eq!(convs.len(), 1);
        let c = &convs[0];
        assert_eq!(c.service, "whatsapp");
        assert_eq!(c.origin, Origin::Notifications);
        assert_eq!(c.access, AccessLevel::Metadatos);
        assert_eq!(c.messages.len(), 1);
        assert_eq!(c.messages[0].kind, "notificacion");
        assert!(c.messages[0].text.contains("hola"));
        assert!(c.messages[0].verified);
        // Y las de Signal, con su propio paquete.
        let signal = notifications_conversations(
            "signal",
            "org.thoughtcrime.securesms",
            DUMPSYS_FIXTURE,
            &Cursor::default(),
        );
        assert_eq!(signal.len(), 1);
        assert_eq!(signal[0].service, "signal");
        assert!(signal[0].messages[0].text.contains("mañana"));
    }

    #[test]
    fn las_notificaciones_respetan_el_cursor() {
        // Cursor por delante de todo: no debe entrar ninguna notificación.
        let convs = notifications_conversations(
            "whatsapp",
            "com.whatsapp",
            DUMPSYS_FIXTURE,
            &Cursor {
                last_ts: 4_000_000_000,
                last_id: "z".into(),
                ..Default::default()
            },
        );
        assert_eq!(convs.len(), 0);
    }

    // --------------------------------------------------------- signal --

    #[test]
    fn signal_esta_cifrado_y_no_se_intenta_descifrar() {
        let rep = signal_db_state(&vec![0x11u8; 40], "signal.db");
        assert_eq!(rep.status, "cifrado");
        assert_eq!(rep.access, AccessLevel::CifradoNoDescifrado);
        assert_eq!(rep.conversations.len(), 0);
        assert_eq!(rep.messages(), 0);
        assert!(rep.notes[0].contains("AndroidKeyStore"));
    }

    // -------------------------------------------------------- discord --

    #[test]
    fn la_exportacion_de_discord_se_normaliza() {
        let rep = parse_discord_export(DISCORD_EXPORT_FIXTURE, &Cursor::default(), 100);
        assert_eq!(rep.status, "ok");
        assert_eq!(rep.access, AccessLevel::Completo);
        assert_eq!(rep.origin, Origin::Export);
        assert_eq!(rep.conversations.len(), 1);
        let c = &rep.conversations[0];
        assert_eq!(c.service, "discord");
        assert_eq!(c.conversation_id, "111222333");
        assert_eq!(c.title, "MegaDaemon / general");
        assert_eq!(c.messages.len(), 2);
        assert_eq!(c.messages[0].author, "ana");
        assert_eq!(c.messages[0].text, "hola discord");
        assert_eq!(c.messages[0].ts, 1704164645);
        assert_eq!(c.messages[0].id, "1001");
    }

    #[test]
    fn la_exportacion_de_discord_respeta_el_cursor() {
        let cursor = Cursor {
            last_ts: 1704164645,
            last_id: "1001".into(),
            ..Default::default()
        };
        let rep = parse_discord_export(DISCORD_EXPORT_FIXTURE, &cursor, 100);
        assert_eq!(rep.messages(), 1);
        assert_eq!(rep.conversations[0].messages[0].id, "1002");
    }

    #[test]
    fn el_token_de_discord_se_guarda_en_un_fichero_0600() {
        // MEDIDO 2026-11-25 (CI rojo, 2ª vez): este test FIJABA
        // `MEGADAEMON_STATE_DIR`, que es GLOBAL al proceso ⇒ en paralelo
        // otro test leía o borraba este mismo directorio.  Ahora no se toca
        // ninguna variable de entorno: el estado del test es su temporal
        // por hilo (`state_dir::state_dir_de_test()`).
        let dir = std::path::PathBuf::from(crate::cockpit_server::state_dir());
        let _ = fs::remove_dir_all(&dir);

        let res = write_discord_token("token-de-prueba");
        let path = dir.join("sync_chats").join("discord_token");
        let existe = path.is_file();
        let leido = read_discord_token();
        #[cfg(unix)]
        let modo = {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(&path).map(|m| m.permissions().mode() & 0o777).unwrap_or(0)
        };

        assert!(res.is_ok(), "no se pudo guardar el token: {:?}", res);
        assert!(existe, "no se creó el fichero del token");
        assert_eq!(leido.as_deref(), Some("token-de-prueba"));
        #[cfg(unix)]
        assert_eq!(modo, 0o600, "el token debe ser 0600");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_url_de_mensajes_de_discord_lleva_el_cursor() {
        let url = discord_messages_url("123", "456");
        assert!(url.contains("/channels/123/messages"));
        assert!(url.contains("after=456"));
        let url2 = discord_messages_url("123", "");
        assert!(!url2.contains("after="));
    }

    // ----------------------------------------------------------- grok --

    #[test]
    fn grok_parsea_una_exportacion_y_avisa_de_los_endpoints_no_verificados() {
        let export = r#"{"conversations":[{"id":"c1","title":"Sobre Rust","messages":[
            {"role":"user","content":"hola grok","timestamp":"2024-05-06T07:08:09.000+00:00"},
            {"role":"assistant","content":"hola","timestamp":"2024-05-06T07:08:10.000+00:00"}]}]}"#;
        let rep = parse_grok_export(export, &Cursor::default(), 10);
        assert_eq!(rep.status, "ok");
        assert_eq!(rep.access, AccessLevel::Completo);
        assert_eq!(rep.conversations.len(), 1);
        assert_eq!(rep.conversations[0].messages.len(), 2);
        assert_eq!(rep.conversations[0].messages[0].author, "user");
        // El endpoint web está marcado como NO VERIFICADO en el catálogo.
        let spec = spec("grok").unwrap();
        assert!(spec.schema.contains("NO VERIFICADO"));
    }

    // ------------------------------------------------------ catalogo --

    #[test]
    fn el_catalogo_cubre_los_cuatro_chats_y_grok() {
        for s in ["whatsapp", "telegram", "signal", "discord", "grok"] {
            assert!(spec(s).is_some(), "falta el servicio {}", s);
        }
        // Grok es el único servicio de IA nuevo: el resto ya estaba.
        assert!(spec("grok").is_some());
        for s in crate::sync::SERVICES {
            assert!(spec(s).is_some(), "falta el servicio de IA {}", s);
        }
    }

    #[test]
    fn whatsapp_y_signal_declaran_el_limite_de_cifrado() {
        assert_eq!(spec("whatsapp").unwrap().access, AccessLevel::Metadatos);
        assert_eq!(
            spec("signal").unwrap().access,
            AccessLevel::CifradoNoDescifrado
        );
        assert!(spec("signal").unwrap().missing.contains("NO se intenta"));
        assert!(spec("whatsapp").unwrap().missing.contains("AndroidKeyStore"));
    }

    #[test]
    fn cada_servicio_tiene_sus_ordenes_de_medicion() {
        for s in catalog() {
            assert!(!s.measure.is_empty(), "{} sin comandos de medición", s.service);
            for m in s.measure {
                assert!(!m.trim().is_empty());
            }
        }
    }

    #[test]
    fn la_doc_generada_tiene_una_fila_por_servicio() {
        let md = catalog_markdown();
        assert_eq!(md.matches("\n|").count(), catalog().len() + 1);
        assert!(md.contains("cifrado_no_descifrado"));
        let meas = measure_markdown();
        assert!(meas.contains("dumpsys notification --noredact"));
    }

    // --------------------------------------------------------- varios --

    #[test]
    fn una_conversacion_legacy_se_convierte_al_modelo_unificado() {
        let legacy = json!({
            "service": "claude",
            "id": "abc",
            "title": "Conversación",
            "updated_at": 1700000000.0,
            "source": "/data/data/com.anthropic.claude/databases/x.db",
            "messages": [
                {"role": "user", "content": "hola", "ts": 1700000000.0, "id": "m1"},
                {"role": "assistant", "content": "respuesta", "ts": 1700000001.0, "id": "m2"}
            ]
        });
        let c = from_sync_conversation(&legacy, "claude", Origin::LocalDb).unwrap();
        assert_eq!(c.service, "claude");
        assert_eq!(c.conversation_id, "abc");
        assert_eq!(c.access, AccessLevel::Completo);
        assert_eq!(c.origin, Origin::LocalDb);
        assert_eq!(c.messages.len(), 2);
        assert_eq!(c.messages[0].id, "m1");
        assert_eq!(c.messages[0].author, "user");
        assert_eq!(c.messages[0].text, "hola");
        assert_eq!(c.messages[0].kind, "texto");
    }

    #[test]
    fn el_iso8601_de_discord_se_convierte_bien() {
        assert_eq!(parse_iso8601("2024-01-02T03:04:05.000+00:00"), Some(1704164645));
        assert_eq!(parse_iso8601("2024-01-02T04:04:05.000+01:00"), Some(1704164645));
        assert_eq!(parse_iso8601("2024-01-02T03:04:05Z"), Some(1704164645));
        assert_eq!(parse_iso8601("no es una fecha"), None);
    }

    #[test]
    fn el_base64_propio_descodifica_bytes_binarios() {
        let b = decode_base64_bytes("aGVsbG8=").unwrap();
        assert_eq!(b, b"hello");
        // Un binario que NO es UTF-8 (BD cifrada) también se decodifica.
        let raw: Vec<u8> = vec![0x00, 0x01, 0xFE, 0xFF];
        let enc = base64_encode(&raw);
        assert_eq!(decode_base64_bytes(&enc).unwrap(), raw);
    }

    fn base64_encode(data: &[u8]) -> String {
        const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            out.push(TABLE[((n >> 18) & 63) as usize] as char);
            out.push(TABLE[((n >> 12) & 63) as usize] as char);
            out.push(if chunk.len() > 1 {
                TABLE[((n >> 6) & 63) as usize] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                TABLE[(n & 63) as usize] as char
            } else {
                '='
            });
        }
        out
    }
}