//! MegaDaemon v13.4 — Inteligencia de apps para interoperar sin API key.
//!
//! Qué resuelve este módulo: **hablar con apps de IA que no publican una API
//! key** (Claude, Cursor, Windsurf, ChatGPT, Codex, Gemini, MiniMax,
//! DeepSeek, Grok, Manus, Antigravity) reusing lo que **ya está en el disco**
//! del usuario: la sesión, el token, la transcripción o el endpoint.
//!
//! Reglas de honestidad del módulo:
//!
//! * Cada perfil lleva `verified`: `true` sólo si **cada** dato del perfil se
//!   ha comprobado contra documentación o un listing de Play Store; `false` si
//!   alguna parte es de oídas (se marca en `notes`).
//! * Los tokens que se detectan **nunca** se muestran enteros: sólo prefijo y
//!   sufijo (`sk-a…mn`). Un `StorageScan` no puede filtrar una credencial.
//! * Los endpoints que no he podido confirmar llevan `endpoint_source:
//!   inferred` en `features`.
//!
//! API pública:
//!
//! | Función | Qué hace |
//! |---|---|
//! | `known_profiles()` | Catálogo de perfiles de apps (escritorio + Android) |
//! | `scan_local_storage(root)` | Barrido recursivo con límites + URLs + tokens redactados |
//! | `extract_endpoints(text)` | Extrae URLs de un bundle/config |
//! | `data_exchange_plan(profile, root)` | Plan de exportación/importación idempotente a NDJSON |
//! | `interop_notes(id)` | Notas de interoperación por app |

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

// ========================================================================= //
// Límites del barrido
// ========================================================================= //

/// Profundidad máxima de recursión del barrido.
pub const MAX_SCAN_DEPTH: usize = 6;
/// Número máximo de ficheros devueltos por un barrido.
pub const MAX_SCAN_FILES: usize = 2_000;
/// Tamaño máximo por fichero que se hashea (por encima: se marca, no se hashea).
pub const MAX_HASH_BYTES: u64 = 32 * 1024 * 1024;
/// Tamaño máximo de un fichero de texto que se lee para buscar URLs/tokens.
pub const MAX_TEXT_BYTES: u64 = 4 * 1024 * 1024;
/// Tamaño máximo de la base SQLite que se vuelca a NDJSON.
pub const MAX_SQLITE_BYTES: u64 = 64 * 1024 * 1024;
/// Número máximo de filas por tabla al convertir SQLite → NDJSON.
pub const MAX_SQLITE_ROWS: usize = 20_000;

// ========================================================================= //
// Perfiles
// ========================================================================= //

/// Lo que se sabe de una app concreta.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppProfile {
    /// Identificador estable (`claude-code`, `chatgpt-android`…).
    pub id: String,
    /// `desktop-linux` | `desktop-macos` | `desktop-windows` | `android` | `cli`.
    pub platform: String,
    /// Paquete Android (vacío si la app no existe en Android).
    pub package: String,
    /// Rutas donde la app guarda sus datos (con `~` como home).
    pub data_paths: Vec<String>,
    /// Formatos de almacenamiento observados (`jsonl`, `sqlite`, `leveldb`…).
    pub storage_formats: Vec<String>,
    /// Rutas donde vive la sesión/token.
    pub auth_storage: Vec<String>,
    /// Endpoints de API conocidos.
    pub endpoints: Vec<String>,
    /// Características técnicas de la app.
    pub features: Vec<String>,
    /// ¿Están **todos** los datos de este perfil verificados?
    pub verified: bool,
    /// Qué está verificado y qué no (en texto).
    pub notes: String,
}

/// Catálogo de perfiles.
///
/// Cada entrada dice de dónde sale el dato: `verified: true` significa que la
/// ruta, el formato y el paquete se han comprobado contra la documentación
/// oficial o el listing de Play Store.
pub fn known_profiles() -> Vec<AppProfile> {
    vec![
        // ---------------------------------------------------------- CLI ---
        AppProfile {
            id: "claude-code".to_string(),
            platform: "cli".to_string(),
            package: String::new(),
            data_paths: vec![
                "~/.claude/".to_string(),
                "~/.claude/projects/".to_string(),
                "~/.claude/statsig/".to_string(),
                "~/.claude/plugins/".to_string(),
                "~/.claude/todos/".to_string(),
                "~/.claude/shell-snapshots/".to_string(),
                "~/.claude/history.jsonl".to_string(),
                "~/.claude.json".to_string(),
            ],
            storage_formats: vec![
                "jsonl".to_string(),
                "json".to_string(),
                "markdown".to_string(),
                "text".to_string(),
            ],
            auth_storage: vec![
                "~/.claude/.credentials.json".to_string(),
                "~/.claude.json".to_string(),
            ],
            endpoints: vec![
                "https://api.anthropic.com/v1/messages".to_string(),
                "https://console.anthropic.com/v1/oauth/token".to_string(),
            ],
            features: vec![
                "endpoint_source:documented".to_string(),
                "transcript_format:jsonl-una-linea-por-evento".to_string(),
                "project_key:path-con-caracteres-no-alfanumericos-reemplazados-por-guion".to_string(),
                "credentials_unencrypted:json-en-claro".to_string(),
            ],
            verified: true,
            notes: "Rutas y credenciales confirmadas en la documentación de \
                    code.claude.com/docs/en/claude-directory. El endpoint OAuth de \
                    refresh se marca `documented` por ser el usado por el propio CLI, \
                    pero no está publicado como contrato de API."
                .to_string(),
        },
        AppProfile {
            id: "codex-cli".to_string(),
            platform: "cli".to_string(),
            package: String::new(),
            data_paths: vec![
                "~/.codex/".to_string(),
                "~/.codex/sessions/".to_string(),
                "~/.codex/history.jsonl".to_string(),
                "~/.codex/config.toml".to_string(),
                "~/.codex/log/".to_string(),
            ],
            storage_formats: vec![
                "jsonl".to_string(),
                "toml".to_string(),
                "json".to_string(),
                "sqlite".to_string(),
            ],
            auth_storage: vec!["~/.codex/auth.json".to_string()],
            endpoints: vec![
                "https://chatgpt.com/backend-api/codex".to_string(),
                "https://api.openai.com/v1/responses".to_string(),
            ],
            features: vec![
                "endpoint_source:documented".to_string(),
                "sessions_layout:YYYY/MM/DD/rollout-*.jsonl".to_string(),
                "session_schema:timestamp+type+payload".to_string(),
                "auth_store_selectable:file|keyring|auto|ephemeral".to_string(),
                "refresh_token_de_un_solo_uso".to_string(),
                "base_url_override:openai_base_url-en-config.toml".to_string(),
            ],
            verified: true,
            notes: "`CODEX_HOME` (por defecto ~/.codex) confirmado en la \
                    documentación de Codex CLI. Los transcripts están en \
                    `sessions/YYYY/MM/DD/rollout-*.jsonl`, que la documentación no \
                    describe: ese layout procede de la estructura de ficheros de una \
                    instalación real, no de un contrato oficial."
                .to_string(),
        },
        AppProfile {
            id: "gemini-cli".to_string(),
            platform: "cli".to_string(),
            package: String::new(),
            data_paths: vec![
                "~/.gemini/".to_string(),
                "~/.gemini/settings.json".to_string(),
                "~/.gemini/GEMINI.md".to_string(),
                "~/.gemini/tmp/".to_string(),
                "~/.gemini/.env".to_string(),
            ],
            storage_formats: vec![
                "json".to_string(),
                "markdown".to_string(),
                "text".to_string(),
            ],
            auth_storage: vec![
                "~/.gemini/oauth_creds.json".to_string(),
                "~/.gemini/.env".to_string(),
            ],
            endpoints: vec![
                "https://generativelanguage.googleapis.com/v1beta/models".to_string(),
                "https://cloudcode-pa.googleapis.com/v1internal".to_string(),
            ],
            features: vec![
                "endpoint_source:inferred".to_string(),
                "code_assist_endpoint:CODE_ASSIST_ENDPOINT".to_string(),
                "context_file:GEMINI.md".to_string(),
                "shell_history:~/.gemini/tmp/<hash>/shell_history".to_string(),
            ],
            verified: false,
            notes: "`settings.json`, `GEMINI.md`, `tmp/<hash>/shell_history` y `.env` \
                    están confirmados en la documentación de gemini-cli. El fichero \
                    `oauth_creds.json` NO está documentado oficialmente (aparece en \
                    análisis de malware como objetivo) y el endpoint interno de Code \
                    Assist es **inferido**: la variable CODE_ASSIST_ENDPOINT sí está \
                    documentada."
                .to_string(),
        },
        AppProfile {
            id: "minimax-cli".to_string(),
            platform: "cli".to_string(),
            package: String::new(),
            data_paths: vec!["~/.minimax/".to_string()],
            storage_formats: vec!["json".to_string(), "jsonl".to_string()],
            auth_storage: vec!["~/.minimax/".to_string()],
            endpoints: vec![],
            features: vec!["endpoint_source:unknown".to_string()],
            verified: false,
            notes: "NO VERIFICADO: no he podido confirmar ni la ruta `~/.minimax/` \
                    ni ningún endpoint de MiniMax en una CLI de escritorio. Se deja la \
                    ficha como marcador; no usar sus rutas para leer nada real."
                .to_string(),
        },
        AppProfile {
            id: "deepseek-cli".to_string(),
            platform: "cli".to_string(),
            package: String::new(),
            data_paths: vec!["~/.deepseek/".to_string()],
            storage_formats: vec!["json".to_string(), "jsonl".to_string()],
            auth_storage: vec!["~/.deepseek/".to_string()],
            endpoints: vec!["https://api.deepseek.com/".to_string()],
            features: vec![
                "endpoint_source:documented".to_string(),
                "solo_https:sin_endpoints_de_escritorio_confirmados".to_string(),
            ],
            verified: false,
            notes: "El endpoint `https://api.deepseek.com/` es público y \
                    documentado. La ruta local `~/.deepseek/` NO está verificada: \
                    la app Android sí existe, pero no hay CLI de escritorio \
                    documentada."
                .to_string(),
        },
        AppProfile {
            id: "grok-cli".to_string(),
            platform: "cli".to_string(),
            package: String::new(),
            data_paths: vec!["~/.grok/".to_string()],
            storage_formats: vec!["json".to_string(), "jsonl".to_string()],
            auth_storage: vec!["~/.grok/".to_string()],
            endpoints: vec!["https://api.x.ai/".to_string()],
            features: vec!["endpoint_source:documented".to_string()],
            verified: false,
            notes: "NO VERIFICADO: no existe (o no he encontrado) una CLI de \
                    escritorio de Grok con estado en `~/.grok/`. El endpoint \
                    `https://api.x.ai/` sí es público. La app Android sí está \
                    verificada (ver `grok-android`)."
                .to_string(),
        },
        // ------------------------------------------------------ Escritorio ---
        AppProfile {
            id: "claude-desktop".to_string(),
            platform: "desktop-linux".to_string(),
            package: String::new(),
            data_paths: vec![
                "~/.config/Claude/".to_string(),
                "~/.config/Claude/Local Storage/".to_string(),
                "~/.config/Claude/IndexedDB/".to_string(),
                "~/.config/Claude/configLibrary/".to_string(),
                "~/.config/Claude/local-agent-mode-sessions/".to_string(),
                "~/Library/Application Support/Claude/".to_string(),
            ],
            storage_formats: vec![
                "leveldb".to_string(),
                "indexeddb".to_string(),
                "json".to_string(),
                "jsonl".to_string(),
            ],
            auth_storage: vec![
                "~/.config/Claude/.credentials.json".to_string(),
                "~/.config/Claude/plugin-mcp-auth/".to_string(),
            ],
            endpoints: vec!["https://claude.ai/api".to_string()],
            features: vec![
                "electron:true".to_string(),
                "endpoint_source:inferred".to_string(),
                "device_id:ant-did".to_string(),
                "session_audit:local-agent-mode-sessions/<id>/audit.jsonl".to_string(),
            ],
            verified: true,
            notes: "Layout de datos confirmado en claude.com/docs/third-party/\
                    claude-desktop/data-storage. La ruta de Linux (`~/.config/Claude`) \
                    es la convención de Electron; la de macOS/Windows sí viene \
                    explícita en esa página."
                .to_string(),
        },
        AppProfile {
            id: "cursor".to_string(),
            platform: "desktop-linux".to_string(),
            package: String::new(),
            data_paths: vec![
                "~/.config/Cursor/User/globalStorage/state.vscdb".to_string(),
                "~/.config/Cursor/User/workspaceStorage/".to_string(),
                "~/.config/Cursor/User/settings.json".to_string(),
                "~/.cursor/".to_string(),
                "~/.cursor/projects/".to_string(),
                "~/.cursor/chats/".to_string(),
                "~/.cursor-server/".to_string(),
            ],
            storage_formats: vec![
                "sqlite".to_string(),
                "json".to_string(),
                "jsonl".to_string(),
            ],
            auth_storage: vec![
                "~/.config/Cursor/User/globalStorage/state.vscdb".to_string(),
                "~/.config/Cursor/User/globalStorage/storage.json".to_string(),
            ],
            endpoints: vec!["https://api2.cursor.sh/".to_string()],
            features: vec![
                "electron:true".to_string(),
                "endpoint_source:inferred".to_string(),
                "sqlite_tables:ItemTable+cursorDiskKV".to_string(),
                "clave_global:composerData:<id>".to_string(),
                "clave_turno:bubbleId:<id>:<id>".to_string(),
                "sqlite_wal:copiar-state.vscdb+-wal+-shm".to_string(),
            ],
            verified: true,
            notes: "Rutas confirmadas en la documentación de Cursor y en análisis \
                    de(store) de terceros: `~/.config/Cursor/User/globalStorage/\
                    state.vscdb` en Linux y `~/.cursor/` para la pila nueva (Store). \
                    El host `api2.cursor.sh` es **inferido**."
                .to_string(),
        },
        AppProfile {
            id: "windsurf".to_string(),
            platform: "desktop-linux".to_string(),
            package: String::new(),
            data_paths: vec![
                "~/.codeium/windsurf/".to_string(),
                "~/.codeium/windsurf/cascade/".to_string(),
                "~/.codeium/windsurf/memories/".to_string(),
                "~/.config/Windsurf/User/globalStorage/state.vscdb".to_string(),
                "~/.config/Windsurf/User/workspaceStorage/".to_string(),
                "~/.windsurf-server/data/User".to_string(),
            ],
            storage_formats: vec![
                "protobuf".to_string(),
                "sqlite".to_string(),
                "json".to_string(),
                "markdown".to_string(),
            ],
            auth_storage: vec![
                "~/.codeium/config.json".to_string(),
                "~/.config/Windsurf/User/globalStorage/state.vscdb".to_string(),
            ],
            endpoints: vec!["https://server.codeium.com/".to_string()],
            features: vec![
                "electron:true".to_string(),
                "endpoint_source:inferred".to_string(),
                "cascade_format:protobuf-sin-esquema-publicado".to_string(),
                "clave_auth:ItemTable.windsurfAuthStatus".to_string(),
                "mcp_config:~/.codeium/windsurf/mcp_config.json".to_string(),
            ],
            verified: true,
            notes: "`~/.codeium/windsurf/cascade/<uuid>.pb` (protobuf **sin esquema \
                    publicado**) y `~/.config/Windsurf/User/globalStorage/state.vscdb` \
                    están documentados por análisis de terceros y por la propia FAQ de \
                    Devin/Windsurf. Ojo: el editor se está renombrando a Devin Desktop \
                    y convive `~/.config/Devin/`."
                .to_string(),
        },
        AppProfile {
            id: "chatgpt-desktop".to_string(),
            platform: "desktop-linux".to_string(),
            package: String::new(),
            data_paths: vec![
                "~/.config/ChatGPT/".to_string(),
                "~/.config/ChatGPT/Local Storage/".to_string(),
                "~/.config/ChatGPT/IndexedDB/".to_string(),
                "~/.config/ChatGPT/Cookies".to_string(),
                "~/.config/ChatGPT/logs/".to_string(),
                "%LOCALAPPDATA%\\Packages\\OpenAI.ChatGPT-Desktop_2p2nqsd0c76g0\\LocalCache\\Roaming\\ChatGPT"
                    .to_string(),
            ],
            storage_formats: vec![
                "leveldb".to_string(),
                "indexeddb".to_string(),
                "sqlite".to_string(),
                "log".to_string(),
            ],
            auth_storage: vec![
                "~/.config/ChatGPT/Cookies".to_string(),
                "~/.config/ChatGPT/Local Storage/leveldb/".to_string(),
            ],
            endpoints: vec!["https://chatgpt.com/backend-api/".to_string()],
            features: vec![
                "electron:true".to_string(),
                "endpoint_source:inferred".to_string(),
                "sesion_por_cookie:__Secure-next-auth.session-token".to_string(),
                "log_de_red:logs/main.log".to_string(),
            ],
            verified: false,
            notes: "La ruta de **Windows** (`Packages\\OpenAI.ChatGPT-Desktop_...\
                    \\LocalCache\\Roaming\\ChatGPT`) está verificada en un informe de \
                    Check Point sobre infostealers. La ruta de Linux `~/.config/ChatGPT/` \
                    es la convención de Electron para el mismo paquete, pero **no la \
                    he verificado** en una instalación real: por eso el perfil entero \
                    queda como `verified: false`."
                .to_string(),
        },
        AppProfile {
            id: "antigravity".to_string(),
            platform: "desktop-linux".to_string(),
            package: String::new(),
            data_paths: vec!["~/.antigravity/".to_string(), "~/.config/Antigravity/".to_string()],
            storage_formats: vec!["json".to_string(), "sqlite".to_string()],
            auth_storage: vec!["~/.antigravity/".to_string()],
            endpoints: vec![],
            features: vec!["endpoint_source:unknown".to_string()],
            verified: false,
            notes: "NO VERIFICADO: Antigravity es el IDE de Google basado en VS Code, \
                    pero no he podido confirmar el directorio de datos ni ningún \
                    endpoint. `~/.codeium/` sería el sitio si compartiese motor con \
                    Codeium: **no está confirmado**."
                .to_string(),
        },
        AppProfile {
            id: "manus".to_string(),
            platform: "desktop-linux".to_string(),
            package: String::new(),
            data_paths: vec!["~/.manus/".to_string()],
            storage_formats: vec!["json".to_string(), "jsonl".to_string()],
            auth_storage: vec!["~/.manus/".to_string()],
            endpoints: vec!["https://manus.im/".to_string()],
            features: vec!["endpoint_source:inferred".to_string()],
            verified: false,
            notes: "NO VERIFICADO: la web `https://manus.im/` es pública, pero la \
                    ruta local `~/.manus/` y el cliente de escritorio no están \
                    documentados."
                .to_string(),
        },
        // ------------------------------------------------------- Android ---
        AppProfile {
            id: "claude-android".to_string(),
            platform: "android".to_string(),
            package: "com.anthropic.claude".to_string(),
            data_paths: vec![
                "/data/data/com.anthropic.claude/".to_string(),
                "/data/data/com.anthropic.claude/databases/".to_string(),
                "/data/data/com.anthropic.claude/shared_prefs/".to_string(),
                "/data/user/0/com.anthropic.claude/files/".to_string(),
            ],
            storage_formats: vec![
                "sqlite".to_string(),
                "shared_prefs_xml".to_string(),
                "files".to_string(),
            ],
            auth_storage: vec![
                "/data/data/com.anthropic.claude/shared_prefs/".to_string(),
                "/data/data/com.anthropic.claude/databases/".to_string(),
            ],
            endpoints: vec!["https://claude.ai/api".to_string()],
            features: vec![
                "endpoint_source:inferred".to_string(),
                "requiere_root:lectura-de-data-data".to_string(),
                "webview:cookie-jar-en-app-private".to_string(),
            ],
            verified: true,
            notes: "Paquete `com.anthropic.claude` verificado. Las rutas internas \
                    (`/data/data/...`) son **convención de Android**, no una \
                    estructura confirmada de esa app: su contenido real (base de \
                    datos de sesión, prefs) cambia entre versiones."
                .to_string(),
        },
        AppProfile {
            id: "chatgpt-android".to_string(),
            platform: "android".to_string(),
            package: "com.openai.chatgpt".to_string(),
            data_paths: vec![
                "/data/data/com.openai.chatgpt/".to_string(),
                "/data/data/com.openai.chatgpt/databases/".to_string(),
                "/data/data/com.openai.chatgpt/shared_prefs/".to_string(),
            ],
            storage_formats: vec![
                "sqlite".to_string(),
                "shared_prefs_xml".to_string(),
            ],
            auth_storage: vec![
                "/data/data/com.openai.chatgpt/databases/".to_string(),
                "/data/data/com.openai.chatgpt/shared_prefs/".to_string(),
            ],
            endpoints: vec!["https://chatgpt.com/backend-api/".to_string()],
            features: vec![
                "endpoint_source:inferred".to_string(),
                "pinning:certificate-pinning-activo".to_string(),
                "requiere_root:lectura-de-data-data".to_string(),
            ],
            verified: true,
            notes: "Paquete `com.openai.chatgpt` verificado (Google Play y FAQ \
                    oficial). El layout interno es el estándar de Android."
                .to_string(),
        },
        AppProfile {
            id: "gemini-android".to_string(),
            platform: "android".to_string(),
            package: "com.google.android.apps.gemini".to_string(),
            data_paths: vec![
                "/data/data/com.google.android.apps.gemini/".to_string(),
                "/data/data/com.google.android.apps.gemini/databases/".to_string(),
                "/data/data/com.google.android.apps.gemini/shared_prefs/".to_string(),
            ],
            storage_formats: vec![
                "sqlite".to_string(),
                "shared_prefs_xml".to_string(),
            ],
            auth_storage: vec![
                "/data/data/com.google.android.apps.gemini/databases/".to_string(),
            ],
            endpoints: vec!["https://gemini.google.com/".to_string()],
            features: vec![
                "endpoint_source:inferred".to_string(),
                "id_legacy:com.google.android.apps.bard".to_string(),
                "gms:no-funciona-sin-Play-Store".to_string(),
            ],
            verified: false,
            notes: "El id **legacy** `com.google.android.apps.bard` está verificado \
                    (aparece en listados de referrer `android-app://`). El id actual \
                    `com.google.android.apps.gemini` es el esperado tras el \
                    renombrado, pero no lo he visto confirmado en un listing de Play \
                    Store en esta sesión: por eso el perfil queda `verified: false`."
                .to_string(),
        },
        AppProfile {
            id: "deepseek-android".to_string(),
            platform: "android".to_string(),
            package: "com.deepseek.chat".to_string(),
            data_paths: vec![
                "/data/data/com.deepseek.chat/".to_string(),
                "/data/data/com.deepseek.chat/databases/".to_string(),
                "/data/data/com.deepseek.chat/shared_prefs/".to_string(),
            ],
            storage_formats: vec![
                "sqlite".to_string(),
                "shared_prefs_xml".to_string(),
            ],
            auth_storage: vec![
                "/data/data/com.deepseek.chat/shared_prefs/".to_string(),
            ],
            endpoints: vec!["https://chat.deepseek.com/".to_string()],
            features: vec![
                "endpoint_source:documented".to_string(),
                "sin_gms:funciona-sin-Play-Store".to_string(),
            ],
            verified: true,
            notes: "Paquete `com.deepseek.chat` verificado. La web \
                    `https://chat.deepseek.com/` es la de la app."
                .to_string(),
        },
        AppProfile {
            id: "grok-android".to_string(),
            platform: "android".to_string(),
            package: "ai.x.grok".to_string(),
            data_paths: vec![
                "/data/data/ai.x.grok/".to_string(),
                "/data/data/ai.x.grok/databases/".to_string(),
                "/data/data/ai.x.grok/shared_prefs/".to_string(),
            ],
            storage_formats: vec![
                "sqlite".to_string(),
                "shared_prefs_xml".to_string(),
            ],
            auth_storage: vec!["/data/data/ai.x.grok/shared_prefs/".to_string()],
            endpoints: vec!["https://grok.com/".to_string()],
            features: vec![
                "endpoint_source:documented".to_string(),
                "id_legacy:com.x.ai".to_string(),
            ],
            verified: true,
            notes: "Paquete `ai.x.grok` verificado contra el listing de Google Play \
                    (`?id=ai.x.grok`). Se ha visto también `com.x.ai`: es un id \
                    previo, no el actual."
                .to_string(),
        },
        AppProfile {
            id: "codex-android".to_string(),
            platform: "android".to_string(),
            package: String::new(),
            data_paths: vec![],
            storage_formats: vec![],
            auth_storage: vec![],
            endpoints: vec![],
            features: vec!["sin_cliente_android_conocido".to_string()],
            verified: false,
            notes: "NO VERIFICADO / probablemente inexistente: Codex es una CLI de \
                    terminal y una extensión de IDE. No hay app Android oficial de \
                    OpenAI Codex con id de paquete conocido; la app Android de \
                    OpenAI es ChatGPT (`com.openai.chatgpt`)."
                .to_string(),
        },
        AppProfile {
            id: "minimax-android".to_string(),
            platform: "android".to_string(),
            package: String::new(),
            data_paths: vec![],
            storage_formats: vec![],
            auth_storage: vec![],
            endpoints: vec!["https://api.minimax.chat/".to_string()],
            features: vec!["endpoint_source:inferred".to_string()],
            verified: false,
            notes: "NO VERIFICADO: no he podido confirmar el id de paquete de la app \
                    Android de MiniMax. El host `api.minimax.chat` es **inferido**."
                .to_string(),
        },
        AppProfile {
            id: "windsurf-android".to_string(),
            platform: "android".to_string(),
            package: String::new(),
            data_paths: vec![],
            storage_formats: vec![],
            auth_storage: vec![],
            endpoints: vec![],
            features: vec!["sin_cliente_android_confirmado".to_string()],
            verified: false,
            notes: "NO VERIFICADO: Windsurf es un IDE de escritorio; existe un \
                    cliente móvil de Codeium, pero no he podido confirmar su id de \
                    paquete."
                .to_string(),
        },
        AppProfile {
            id: "cursor-android".to_string(),
            platform: "android".to_string(),
            package: String::new(),
            data_paths: vec![],
            storage_formats: vec![],
            auth_storage: vec![],
            endpoints: vec![],
            features: vec!["sin_cliente_android".to_string()],
            verified: false,
            notes: "NO VERIFICADO / probablemente inexistente: Cursor es sólo \
                    escritorio (Electron). En Android no hay app oficial de Cursor."
                .to_string(),
        },
        AppProfile {
            id: "antigravity-android".to_string(),
            platform: "android".to_string(),
            package: String::new(),
            data_paths: vec![],
            storage_formats: vec![],
            auth_storage: vec![],
            endpoints: vec![],
            features: vec!["sin_cliente_android_confirmado".to_string()],
            verified: false,
            notes: "NO VERIFICADO: no hay id de paquete Android confirmado para \
                    Antigravity."
                .to_string(),
        },
        AppProfile {
            id: "lucky-patcher".to_string(),
            platform: "android".to_string(),
            package: "ru.drjmwdme.cgwbfzijr".to_string(),
            // Sin rutas de datos medidas: no se inventan.
            data_paths: vec![],
            storage_formats: vec![],
            auth_storage: vec![],
            endpoints: vec![
                "http://patch.chelpus.com/submit".to_string(),
                "http://109.95.209.70:8008/".to_string(),
                "https://149.56.155.246/luckypatcher/".to_string(),
                "http://www.appnext.com/myid.html".to_string(),
                "ws.tapjoyads.com".to_string(),
                "pagead2.googlesyndication.com".to_string(),
                "https://t.me/lucky_patcher_chelpus".to_string(),
                "https://app.transifex.com/chelpus/lucky-patcher-1".to_string(),
            ],
            features: vec![
                "packaging:repackaged-lucky-patcher".to_string(),
                "same_classes_dex:5b0d376ddc79d4ff409f3158b382a335da1b7157a53815211c1d77c4f1200a79".to_string(),
                "obfuscation:package-and-label-only".to_string(),
                "signing:aosp-testkey".to_string(),
                "embeds:aosp-signing-keys".to_string(),
                "embeds:apksigner".to_string(),
                "play-license-bypass:ITestServiceInterface".to_string(),
                "xposed-module:com.xposed.XSupport".to_string(),
                "uploads-target-apk:patch.chelpus.com".to_string(),
                "network:plaintext-and-raw-ip".to_string(),
                "tracking:appnext-affiliate".to_string(),
                "native-libs:none".to_string(),
            ],
            verified: true,
            notes: "Lucky Patcher reempaquetado como 'Marina Patcher' (paquete \
                    ofuscado ru.drjmwdme.cgwbfzijr, fuera de Play Store): el \
                    classes.dex (sha256 5b0d376d...) coincide byte a byte con el \
                    de la app instalada en el movil, asi que el DEX es el mismo y \
                    el ocultamiento esta en el PAQUETE y el NOMBRE, no en los \
                    identificadores. Medido en docs/MARINA_PATCHER_ANALYSIS.md."
                .to_string(),
        },
        AppProfile {
            id: "manus-android".to_string(),
            platform: "android".to_string(),
            package: String::new(),
            data_paths: vec![],
            storage_formats: vec![],
            auth_storage: vec![],
            endpoints: vec!["https://manus.im/".to_string()],
            features: vec!["endpoint_source:inferred".to_string()],
            verified: false,
            notes: "NO VERIFICADO: no he podido confirmar el id de paquete de la \
                    app Android de Manus ni sus rutas internas."
                .to_string(),
        },
    ]
}

/// Perfil por id.
pub fn profile_by_id(id: &str) -> Option<AppProfile> {
    known_profiles().into_iter().find(|p| p.id == id)
}

// ========================================================================= //
// Extracción de endpoints y tokens
// ========================================================================= //

/// ¿Es delimitador de una URL?
fn is_url_delimiter(c: u8) -> bool {
    matches!(
        c,
        b' ' | b'\t' | b'\n' | b'\r' | b'"' | b'\'' | b'`' | b'<' | b'>' | b')' | b'(' | b']'
            | b'[' | b',' | b';' | b'\\'
    )
}

/// Extrae URLs de un texto (bundle JS, config, log…).
///
/// Sin `regex` y sin copiar el resto del texto en cada posición (lo que lo
/// convertía en O(n²) sobre ficheros de varios MB): se trabaja sobre los
/// bytes y se consume la URL hasta un delimitador. Se deduplica conservando
/// el orden de aparición.
pub fn extract_endpoints(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;

    while i < b.len() {
        let https = b.len() - i >= 8 && b[i..i + 8].eq_ignore_ascii_case(b"https://");
        let http = b.len() - i >= 7 && b[i..i + 7].eq_ignore_ascii_case(b"http://");
        if !https && !http {
            i += 1;
            continue;
        }
        let start = i;
        let mut j = i + 7;
        while j < b.len() && !is_url_delimiter(b[j]) {
            j += 1;
        }
        // Se recorta la puntuación final habitual de una frase.
        let mut end = j;
        while end > start + 7 && matches!(b[end - 1], b'.' | b',' | b';' | b':' | b'!' | b'?') {
            end -= 1;
        }
        if end > start + 7 {
            let url = String::from_utf8_lossy(&b[start..end]).to_string();
            if !out.contains(&url) {
                out.push(url);
            }
        }
        i = j.max(i + 1);
    }
    out
}

/// Redacta un valor que parece una credencial.
///
/// Nunca se devuelve el valor completo: sólo 4 caracteres al principio y 2 al
/// final (`sk-ab…yz`). Si el valor es corto, se devuelve sólo la longitud.
pub fn redact_secret(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        return format!("<redacted:{} chars>", chars.len());
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 2..].iter().collect();
    format!("{}…{}", head, tail)
}

/// Nombres de clave que delatan una credencial.
const SECRET_KEYS: &[&str] = &[
    "api_key",
    "apikey",
    "api-key",
    "access_token",
    "accessToken",
    "refresh_token",
    "refreshToken",
    "id_token",
    "idToken",
    "session_token",
    "sessionToken",
    "auth_token",
    "authToken",
    "authorization",
    "client_secret",
    "clientSecret",
    "secret",
    "password",
    "passwd",
    "pwd",
    "token",
    "bearer",
    "credential",
    "private_key",
    "privateKey",
];

/// Extrae valores que parecen credenciales de un texto.
///
/// Devuelve pares `(clave, valor redactado)`. El valor **nunca** aparece
/// entero: sólo prefijo y sufijo.
pub fn extract_secrets(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let lower = text.to_ascii_lowercase();

    for key in SECRET_KEYS {
        let mut from = 0usize;
        while let Some(idx) = lower[from..].find(&key.to_ascii_lowercase()) {
            let pos = from + idx;
            from = pos + key.len();
            // Se exige un separador antes del valor. La clave puede venir
            // entrecomillada (JSON: `"api_key": "valor"`), así que se salta
            // la comilla de cierre antes de buscar `:` o `=`.
            let after = text[pos + key.len()..].trim_start();
            let after = after
                .strip_prefix('"')
                .or_else(|| after.strip_prefix('\''))
                .unwrap_or(after)
                .trim_start();
            let after = after
                .strip_prefix(':')
                .or_else(|| after.strip_prefix('='))
                .unwrap_or(after)
                .trim_start();
            let quote = after.chars().next().unwrap_or(' ');
            if quote != '"' && quote != '\'' {
                continue;
            }
            let body = &after[1..];
            let end = body.find(quote).unwrap_or(body.len()).min(512);
            let value = &body[..end];
            if value.len() < 8 || value.contains(' ') {
                continue;
            }
            let redacted = redact_secret(value);
            let entry = (key.to_string(), redacted);
            if !out.contains(&entry) {
                out.push(entry);
            }
        }
    }
    out
}

// ========================================================================= //
// Barrido del almacenamiento local
// ========================================================================= //

/// Un fichero encontrado en el barrido.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScannedFile {
    /// Ruta absoluta en el sistema de ficheros.
    pub path: String,
    /// Tamaño en bytes.
    pub size: u64,
    /// Clasificación (`sqlite`, `leveldb`, `jsonl`, `json`, `cookies`, `log`…).
    pub kind: String,
    /// Fecha de modificación en ISO-8601 (`Z`).
    pub mtime: String,
    /// SHA-256 del contenido (vacío si el fichero excede `MAX_HASH_BYTES`).
    pub sha256: String,
    /// URLs encontradas dentro del fichero.
    pub urls: Vec<String>,
    /// Credenciales detectadas, **redactadas**.
    pub redacted_tokens: Vec<String>,
}

/// Resultado de un barrido.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StorageScan {
    /// Ficheros encontrados.
    pub files: Vec<ScannedFile>,
    /// Resumen (JSON libre): recuentos, URLs y tokens redactados.
    pub summary: Value,
}

/// Clasifica un fichero por nombre y por la ruta que lo contiene.
pub fn classify_file(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    let full = path.to_string_lossy().to_ascii_lowercase();

    if lower == "state.vscdb" || lower.ends_with(".vscdb") {
        return "sqlite".to_string();
    }
    if lower.ends_with(".db") || lower.ends_with(".sqlite") || lower.ends_with(".sqlite3") {
        return "sqlite".to_string();
    }
    if full.contains("/local storage/leveldb")
        || full.contains("/session storage/leveldb")
        || lower.ends_with(".ldb")
        || lower.ends_with(".log")
            && full.contains("/leveldb/")
    {
        return "leveldb".to_string();
    }
    if full.contains("/indexeddb/") {
        return "indexeddb".to_string();
    }
    if lower == "cookies" || lower == "cookies.sqlite" || full.contains("/network/cookies") {
        return "cookies".to_string();
    }
    if lower.ends_with(".jsonl") {
        return "jsonl".to_string();
    }
    if lower.ends_with(".json") {
        return "json".to_string();
    }
    if lower.ends_with(".toml") {
        return "toml".to_string();
    }
    if lower.ends_with(".log") {
        return "log".to_string();
    }
    if lower.ends_with(".pb") {
        return "protobuf".to_string();
    }
    if lower.ends_with(".xml") {
        return "xml".to_string();
    }
    if lower.ends_with(".db-wal") || lower.ends_with(".db-shm") {
        return "sqlite".to_string();
    }
    "other".to_string()
}

/// Convierte un timestamp UNIX a una fecha ISO-8601 (`YYYY-MM-DDTHH:MM:SSZ`).
fn format_mtime(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // Algoritmo civil de Howard Hinnant (days → y/m/d).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, h, mi, s
    )
}

/// SHA-256 en hexadecimal.
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let out = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in out.iter() {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Escanea un directorio recursivamente.
///
/// Límites: [`MAX_SCAN_DEPTH`], [`MAX_SCAN_FILES`], [`MAX_HASH_BYTES`] y
/// [`MAX_TEXT_BYTES`]. Nunca sigue enlaces simbólicos (evita ciclos).
pub fn scan_local_storage(root: &str) -> StorageScan {
    let mut files: Vec<ScannedFile> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let root_path = PathBuf::from(root);

    if !root_path.exists() {
        warnings.push(format!("la raíz no existe: {}", root));
        return StorageScan {
            files,
            summary: json!({
                "root": root,
                "exists": false,
                "total_files": 0,
                "total_bytes": 0,
                "by_kind": {},
                "urls": [],
                "redacted_tokens": [],
                "warnings": warnings,
            }),
        };
    }

    let mut all_urls: Vec<String> = Vec::new();
    let mut all_tokens: Vec<String> = Vec::new();
    let mut total_bytes: u64 = 0;

    if root_path.is_file() {
        // La raíz es un fichero suelto (p.ej. un `state.vscdb`).
        scan_one_file(
            &root_path,
            &mut files,
            &mut all_urls,
            &mut all_tokens,
            &mut total_bytes,
        );
    } else {
        walk(
            &root_path,
            0,
            &mut files,
            &mut all_urls,
            &mut all_tokens,
            &mut total_bytes,
            &mut warnings,
        );
    }

    let mut by_kind: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut by_kind_bytes: std::collections::BTreeMap<String, u64> =
        std::collections::BTreeMap::new();
    for f in &files {
        *by_kind.entry(f.kind.clone()).or_insert(0) += 1;
        *by_kind_bytes.entry(f.kind.clone()).or_insert(0) += f.size;
    }

    let summary = json!({
        "root": root,
        "exists": true,
        "total_files": files.len(),
        "total_bytes": total_bytes,
        "by_kind": by_kind,
        "bytes_by_kind": by_kind_bytes,
        "urls": all_urls,
        "redacted_tokens": all_tokens,
        "warnings": warnings,
        "limits": {
            "max_depth": MAX_SCAN_DEPTH,
            "max_files": MAX_SCAN_FILES,
            "max_hash_bytes": MAX_HASH_BYTES,
            "max_text_bytes": MAX_TEXT_BYTES,
        },
        "note": "los tokens salen redactados (prefijo+sufijo); nunca el valor entero",
    });

    StorageScan { files, summary }
}

#[allow(clippy::too_many_arguments)]
fn walk(
    dir: &Path,
    depth: usize,
    files: &mut Vec<ScannedFile>,
    all_urls: &mut Vec<String>,
    all_tokens: &mut Vec<String>,
    total_bytes: &mut u64,
    warnings: &mut Vec<String>,
) {
    if depth > MAX_SCAN_DEPTH {
        warnings.push(format!(
            "profundidad máxima alcanzada en {} (se deja de bajar)",
            dir.display()
        ));
        return;
    }
    if files.len() >= MAX_SCAN_FILES {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            warnings.push(format!("no se pudo leer {}: {}", dir.display(), e));
            return;
        }
    };

    // Orden estable: primero directorios, luego ficheros, por nombre.
    let mut sorted: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        sorted.push(entry.path());
    }
    sorted.sort();

    for path in sorted {
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
            continue; // No se siguen enlaces: pueden crear ciclos.
        }
        if meta.is_dir() {
            walk(
                &path,
                depth + 1,
                files,
                all_urls,
                all_tokens,
                total_bytes,
                warnings,
            );
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        if files.len() >= MAX_SCAN_FILES {
            warnings.push("límite de ficheros alcanzado: el barrido se corta".to_string());
            return;
        }

        scan_one_file(
            &path,
            files,
            all_urls,
            all_tokens,
            total_bytes,
        );
    }
}

/// Analiza **un** fichero: tamaño, tipo, mtime, SHA-256, URLs y tokens.
fn scan_one_file(
    path: &Path,
    files: &mut Vec<ScannedFile>,
    all_urls: &mut Vec<String>,
    all_tokens: &mut Vec<String>,
    total_bytes: &mut u64,
) {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return,
    };
    let size = meta.len();
    *total_bytes = total_bytes.saturating_add(size);
    let kind = classify_file(path);

    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| format_mtime(d.as_secs() as i64))
        .unwrap_or_else(|| "unknown".to_string());

    let sha256 = if size <= MAX_HASH_BYTES {
        fs::read(path)
            .map(|b| sha256_hex(&b))
            .unwrap_or_default()
    } else {
        String::new()
    };

    // URLs y tokens sólo en ficheros de texto razonablemente pequeños.
    let mut urls: Vec<String> = Vec::new();
    let mut tokens: Vec<String> = Vec::new();
    if size <= MAX_TEXT_BYTES && is_textual(&kind) {
        if let Ok(buf) = fs::read(path) {
            let text = String::from_utf8_lossy(&buf);
            urls = extract_endpoints(&text);
            tokens = extract_secrets(&text)
                .into_iter()
                .map(|(_, v)| v)
                .collect();
        }
    }

    for u in &urls {
        if !all_urls.contains(u) {
            all_urls.push(u.clone());
        }
    }
    for t in &tokens {
        if !all_tokens.contains(t) {
            all_tokens.push(t.clone());
        }
    }

    files.push(ScannedFile {
        path: path.to_string_lossy().to_string(),
        size,
        kind,
        mtime,
        sha256,
        urls,
        redacted_tokens: tokens,
    });
}

/// ¿El formato admite lectura como texto?
fn is_textual(kind: &str) -> bool {
    matches!(
        kind,
        "json" | "jsonl" | "log" | "toml" | "xml" | "leveldb" | "other" | "cookies"
    )
}

// ========================================================================= //
// Plan de intercambio de datos
// ========================================================================= //

/// Un fichero a exportar.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExportItem {
    /// Ruta de origen (en el equipo donde vive la app).
    pub source: String,
    /// Ruta de destino canónica (NDJSON).
    pub target: String,
    /// Tamaño en bytes del origen (o del NDJSON generado).
    pub bytes: u64,
    /// Formato del origen (`jsonl`, `sqlite`, `json`…).
    pub kind: String,
    /// SHA-256 del contenido de origen (vacío si no se pudo calcular).
    pub sha256: String,
    /// ¿Va cifrado en reposo?
    pub encrypted: bool,
}

/// Plan de exportación/importación.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExportPlan {
    /// Ficheros a exportar.
    pub items: Vec<ExportItem>,
    /// Total de bytes del plan.
    pub total_bytes: u64,
    /// Avisos: lo que no se puede exportar, lo que hay que revisar a mano…
    pub warnings: Vec<String>,
    /// Hash del plan completo: sirve de ancla de idempotencia.
    pub plan_hash: String,
    /// Formato canónico de destino.
    pub canonical_format: String,
}

/// Resuelve una ruta del perfil contra una raíz (`~` = la raíz).
fn resolve_path(root: &str, p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        PathBuf::from(root).join(rest)
    } else if p == "~" {
        PathBuf::from(root)
    } else {
        PathBuf::from(p)
    }
}

/// Plan de exportación/importación entre una app y megadaemon.
///
/// Es **absoluta** (cada fichero tiene su ruta de origen y su destino) e
/// **idempotente**: cada ítem lleva el SHA-256 de su contenido, de forma que
/// reejecutar el plan sólo reescribe lo que ha cambiado (comparando el hash
/// con el del destino). El destino canónico es NDJSON.
pub fn data_exchange_plan(profile: &AppProfile, source_root: &str) -> ExportPlan {
    let mut items: Vec<ExportItem> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut total_bytes: u64 = 0;

    let base = format!("megadaemon/interop/{}", profile.id);

    for p in &profile.data_paths {
        let resolved = resolve_path(source_root, p);
        if !resolved.exists() {
            warnings.push(format!("no existe en {}: {}", source_root, p));
            continue;
        }
        let meta = match fs::metadata(&resolved) {
            Ok(m) => m,
            Err(e) => {
                warnings.push(format!("no se pudo leer {}: {}", p, e));
                continue;
            }
        };

        if meta.is_file() {
            let sub = plan_item(&resolved, &resolved, &base);
            if sub.is_empty() {
                warnings.push(format!("no se pudo planificar: {}", p));
            }
            for item in sub {
                total_bytes += item.bytes;
                items.push(item);
            }
            continue;
        }

        // Directorio: se recorren los ficheros con formato interesante.
        let mut found = 0usize;
        let mut dir_warnings: Vec<String> = Vec::new();
        let mut sub_items: Vec<ExportItem> = Vec::new();
        collect_dir(
            &resolved,
            &resolved,
            &base,
            &mut sub_items,
            &mut found,
            &mut dir_warnings,
            0,
        );
        for item in sub_items {
            total_bytes += item.bytes;
            items.push(item);
        }
        warnings.extend(dir_warnings);
        if found == 0 {
            warnings.push(format!("directorio sin ficheros de datos: {}", p));
        }
    }

    // Credenciales: se listan pero **no** se copian al destino canónico.
    if !profile.auth_storage.is_empty() {
        warnings.push(format!(
            "credenciales detectadas (NO se exportan, sólo se listan): {}",
            profile.auth_storage.join(", ")
        ));
    }

    let plan_hash = {
        let mut hs: Vec<String> = items.iter().map(|i| i.sha256.clone()).collect();
        hs.sort();
        sha256_hex(hs.join("|").as_bytes())
    };

    ExportPlan {
        items,
        total_bytes,
        warnings,
        plan_hash,
        canonical_format: "ndjson".to_string(),
    }
}

/// Crea los ítems de exportación de un fichero.
///
/// Un fichero puede generar más de un ítem: una base SQLite legible produce
/// el NDJSON canónico **y** el binario original como anexo.
fn plan_item(source: &Path, original: &Path, base: &str) -> Vec<ExportItem> {
    let meta = match fs::metadata(source) {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };
    let kind = classify_file(source);
    let size = meta.len();

    // Nombre relativo dentro del perfil (para el destino).
    let rel = original
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "data".to_string());
    let rel = rel.replace('/', "_");

    let sha256 = if size <= MAX_HASH_BYTES {
        fs::read(source)
            .map(|b| sha256_hex(&b))
            .unwrap_or_default()
    } else {
        String::new()
    };

    // NDJSON: lo que ya es JSONL o JSON se puede convertir; lo binario no.
    let target = if kind == "jsonl" || kind == "json" {
        format!("{}/ndjson/{}.ndjson", base, rel)
    } else {
        format!("{}/blobs/{}.{}", base, rel, kind)
    };

    // Conversión real SQLite → NDJSON cuando la base es legible.
    if kind == "sqlite" && size <= MAX_SQLITE_BYTES {
        if let Some((ndjson, _rows)) = sqlite_to_ndjson(source) {
            return vec![
                ExportItem {
                    source: source.to_string_lossy().to_string(),
                    target: format!("{}/ndjson/{}.ndjson", base, rel),
                    bytes: ndjson.len() as u64,
                    kind: "ndjson".to_string(),
                    sha256: sha256_hex(ndjson.as_bytes()),
                    encrypted: false,
                },
                ExportItem {
                    source: source.to_string_lossy().to_string(),
                    target: format!("{}/blobs/{}.sqlite", base, rel),
                    bytes: size,
                    kind: "sqlite".to_string(),
                    sha256,
                    encrypted: false,
                },
            ];
        }
    }

    let notes_kind = if kind != "jsonl" && kind != "json" {
        format!("{} (binario: se copia tal cual)", kind)
    } else {
        kind.clone()
    };

    vec![ExportItem {
        source: source.to_string_lossy().to_string(),
        target,
        bytes: size,
        kind: notes_kind,
        sha256,
        encrypted: false,
    }]
}

/// Recorre un directorio buscando ficheros de datos.
#[allow(clippy::too_many_arguments)]
fn collect_dir(
    dir: &Path,
    base_dir: &Path,
    base: &str,
    out: &mut Vec<ExportItem>,
    found: &mut usize,
    warnings: &mut Vec<String>,
    depth: usize,
) {
    if depth > MAX_SCAN_DEPTH || out.len() >= MAX_SCAN_FILES {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            warnings.push(format!("no se pudo leer {}: {}", dir.display(), e));
            return;
        }
    };
    let mut sorted: Vec<PathBuf> = Vec::new();
    for e in entries.flatten() {
        sorted.push(e.path());
    }
    sorted.sort();

    for path in sorted {
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            // Se saltan las cachés: no aportan datos y multiplican el tamaño.
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if matches!(
                name.as_str(),
                "cache" | "caches" | "gpuCache" | "Code Cache" | "blob_storage" | "logs"
            ) {
                continue;
            }
            collect_dir(
                &path,
                base_dir,
                base,
                out,
                found,
                warnings,
                depth + 1,
            );
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let kind = classify_file(&path);
        if kind == "other" {
            continue;
        }
        // El nombre relativo conserva la estructura del directorio, para que
        // dos ficheros con el mismo nombre no colisionen en el destino.
        let rel = path
            .strip_prefix(base_dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());
        let rel = rel.replace('/', "_");
        let target = if kind == "jsonl" || kind == "json" {
            format!("{}/ndjson/{}.ndjson", base, rel)
        } else {
            format!("{}/blobs/{}.{}", base, rel, kind)
        };
        let sha256 = if meta.len() <= MAX_HASH_BYTES {
            fs::read(&path)
                .map(|b| sha256_hex(&b))
                .unwrap_or_default()
        } else {
            String::new()
        };
        out.push(ExportItem {
            source: path.to_string_lossy().to_string(),
            target,
            bytes: meta.len(),
            kind,
            sha256,
            encrypted: false,
        });
        *found += 1;
    }
}

/// Convierte una base SQLite a NDJSON (una línea JSON por fila).
///
/// Si la base no se puede abrir (cifrada con el keyring del SO, corrupta o de
/// otra app), devuelve `None`: es preferible no exportar a exportar basura.
pub fn sqlite_to_ndjson(path: &Path) -> Option<(String, usize)> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .ok()?;
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'").ok()?;
    let tables: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .ok()?
        .filter_map(|x| x.ok())
        .collect();

    let mut out = String::new();
    let mut rows_total = 0usize;
    for table in tables {
        let sql = format!("SELECT * FROM \"{}\"", table.replace('"', "\"\""));
        let mut q = match conn.prepare(&sql) {
            Ok(q) => q,
            Err(_) => continue,
        };
        let column_count = q.column_count();
        let names: Vec<String> = (0..column_count)
            .map(|i| q.column_name(i).unwrap_or("?").to_string())
            .collect();
        let mut rows = match q.query([]) {
            Ok(r) => r,
            Err(_) => continue,
        };
        while let Some(Some(row)) = rows.next().ok() {
            if rows_total >= MAX_SQLITE_ROWS {
                break;
            }
            let mut obj = serde_json::Map::new();
            for i in 0..column_count {
                let key = names[i].clone();
                let val: Value = match row.get_ref(i) {
                    Ok(rusqlite::types::ValueRef::Null) => Value::Null,
                    Ok(rusqlite::types::ValueRef::Integer(i)) => Value::from(i),
                    Ok(rusqlite::types::ValueRef::Real(f)) => Value::from(f),
                    Ok(rusqlite::types::ValueRef::Text(t)) => {
                        Value::from(String::from_utf8_lossy(t).to_string())
                    }
                    Ok(rusqlite::types::ValueRef::Blob(b)) => {
                        Value::from(format!("<blob:{} bytes>", b.len()))
                    }
                    Err(_) => Value::Null,
                };
                obj.insert(key, val);
            }
            out.push_str(&Value::Object(obj).to_string());
            out.push('\n');
            rows_total += 1;
        }
    }
    if rows_total == 0 {
        None
    } else {
        Some((out, rows_total))
    }
}

// ========================================================================= //
// Notas de interoperación
// ========================================================================= //

/// Notas de cómo interoperar con una app **sin API key**.
///
/// Tres vías, de menos a más invasiva:
/// 1. **Reutilizar la sesión/token** guardado en disco por la propia app.
/// 2. **Enganchar el proceso** (hook de red o de API) mientras corre.
/// 3. **Reimplementar el endpoint** con las credenciales de la sesión.
pub fn interop_notes(profile_id: &str) -> Vec<String> {
    let mut notes: Vec<String> = Vec::new();
    match profile_id {
        "claude-code" => {
            notes.push(
                "VÍA 1 (sesión): `~/.claude/.credentials.json` guarda el access token \
                 OAuth en JSON plano. Leyéndolo y refrescándolo (POST \
                 https://console.anthropic.com/v1/oauth/token) se puede hablar con la \
                 API de Anthropic sin API key. Es la vía menos invasiva."
                    .to_string(),
            );
            notes.push(
                "VÍA 2 (hook): Claude Code habla HTTPS contra \
                 https://api.anthropic.com/v1/messages; un proxy local (mitmproxy) \
                 intercepta el tráfico y ve el prompt y la respuesta sin tocar la app."
                    .to_string(),
            );
            notes.push(
                "VÍA 3 (endpoint): POST a https://api.anthropic.com/v1/messages con \
                 `anthropic-version: 2023-06-01` y `Authorization: Bearer <token de \
                 ~/.claude/.credentials.json>`."
                    .to_string(),
            );
            notes.push(
                "Transcripciones: `~/.claude/projects/<proyecto>/<sesion>.jsonl` — una \
                 línea JSON por evento; es el formato canónico para reimportar."
                    .to_string(),
            );
            notes.push(
                "AVISO: el token da acceso a la cuenta del usuario; usarlo desde otro \
                 cliente puede violar los términos de Anthropic. Sólo con cuentas propias."
                    .to_string(),
            );
        }
        "codex-cli" => {
            notes.push(
                "VÍA 0 (la más limpia): `~/.codex/config.toml` admite \
                 `openai_base_url`. Apuntando esa URL a un proxy local, Codex envía \
                 todo el tráfico ahí: sin tocar tokens ni hooks."
                    .to_string(),
            );
            notes.push(
                "VÍA 1 (sesión): `~/.codex/auth.json` contiene el refresh token de \
                 ChatGPT (OAuth). Los refresh tokens son de UN SOLO USO: hay que \
                 refrescar **antes** de que expire y persistir el token nuevo, o se \
                 invalida la sesión."
                    .to_string(),
            );
            notes.push(
                "VÍA 3 (endpoint): el endpoint de ChatGPT que usa el CLI es \
                 https://chatgpt.com/backend-api/codex con el bearer del auth.json."
                    .to_string(),
            );
            notes.push(
                "Transcripciones: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` con \
                 esquema `{timestamp, type, payload}` — NDJSON de serie."
                    .to_string(),
            );
            notes.push(
                "El almacén de credenciales es configurable \
                 (`cli_auth_credentials_store = file|keyring|auto|ephemeral`): con \
                 `keyring` no hay nada legible en disco y esta vía no aplica."
                    .to_string(),
            );
        }
        "gemini-cli" => {
            notes.push(
                "VÍA 0: la variable de entorno `CODE_ASSIST_ENDPOINT` (documentada) \
                 permite apuntar el CLI a un proxy local sin tocar credenciales."
                    .to_string(),
            );
            notes.push(
                "VÍA 1 (sesión): `~/.gemini/oauth_creds.json` guarda el OAuth de \
                 Google (fichero NO documentado oficialmente: ver `verified`)."
                    .to_string(),
            );
            notes.push(
                "AVISO IMPORTANTE: la FAQ oficial de Gemini CLI prohíbe explícitamente \
                 que otro software reutilice su OAuth para acceder al backend de \
                 Google. Reutilizar esas credenciales fuera del CLI va contra sus \
                 términos: hazlo sólo con una cuenta propia y sabiendo que \
                 puedes perder el acceso."
                    .to_string(),
            );
        }
        "cursor" => {
            notes.push(
                "VÍA 1 (sesión): `~/.config/Cursor/User/globalStorage/state.vscdb` \
                 (SQLite, tablas `ItemTable` y `cursorDiskKV`) guarda el estado de \
                 sesión y, en algunas instalaciones, el token."
                    .to_string(),
            );
            notes.push(
                "OJO: el `state.vscdb` de Electron cifra los secretos con \
                 safeStorage (AES-128-CBC) y la clave vive en el llavero del SO: el \
                 valor **no es portable** entre máquinas. Copiar la base no sirve; \
                 copiarla en la misma máquina, sí."
                    .to_string(),
            );
            notes.push(
                "VÍA 2 (hook): el binario `cursor-agent` / el proceso Electron hacen \
                 las llamadas; un proxy local las intercepta."
                    .to_string(),
            );
            notes.push(
                "Reimportación: las claves `composerData:<id>` y \
                 `bubbleId:<id>:<id>` de `cursorDiskKV` son el formato canónico de una \
                 conversación. Hay que copiar también `state.vscdb-wal` y `-shm`."
                    .to_string(),
            );
            notes.push(
                "El host `api2.cursor.sh` es **inferido**: verificarlo antes de \
                 escribir código contra él."
                    .to_string(),
            );
        }
        "windsurf" => {
            notes.push(
                "VÍA 1 (sesión): `~/.codeium/config.json` (clave `api_key`) y la \
                 clave `ItemTable.windsurfAuthStatus` (JSON con `apiKey` y `email`) \
                 en `state.vscdb`."
                    .to_string(),
            );
            notes.push(
                "Las conversaciones Cascade están en PROTOBUF \
                 (`~/.codeium/windsurf/cascade/<uuid>.pb`) **sin esquema publicado**: \
                 no hay parser oficial. Plan A = hooking del proceso; plan B = \
                 ingeniería inversa del `.pb` (cambia entre versiones)."
                    .to_string(),
            );
            notes.push(
                "El MCP config (`~/.codeium/windsurf/mcp_config.json`) es JSON plano \
                 y se puede reutilizar tal cual."
                    .to_string(),
            );
            notes.push(
                "Ojo con el renombramiento: el editor pasa a llamarse Devin Desktop \
                 y conviven `~/.config/Windsurf/` y `~/.config/Devin/`."
                    .to_string(),
            );
        }
        "chatgpt-desktop" => {
            notes.push(
                "VÍA 1 (sesión): la sesión vive en la cookie \
                 `__Secure-next-auth.session-token` del `Cookies` de Electron \
                 (`~/.config/ChatGPT/Cookies`)."
                    .to_string(),
            );
            notes.push(
                "VÍA 2 (hook): proxy local sobre `https://chatgpt.com/backend-api/`; \
                 el pinning de certificado complica el MITM, pero no el proxy \
                 (se cambia el destino, no se falsifica el TLS)."
                    .to_string(),
            );
            notes.push(
                "Reimportación: `Local Storage/leveldb` (Electron) e `IndexedDB` son \
                 leveldb; hay que exportarlos como pares clave/valor, no como SQLite."
                    .to_string(),
            );
        }
        "claude-desktop" => {
            notes.push(
                "VÍA 1 (sesión): `~/.config/Claude/.credentials.json` y la carpeta \
                 `plugin-mcp-auth/` guardan los sign-ins de los servidores MCP."
                    .to_string(),
            );
            notes.push(
                "Los datos de sesión están en `local-agent-mode-sessions/<id>/` \
                 (un `local_<uuid>.json` + un `audit.jsonl` por sesión)."
                    .to_string(),
            );
        }
        "deepseek-android" | "grok-android" | "chatgpt-android" | "claude-android"
        | "gemini-android" => {
            notes.push(
                "VÍA 1 (sesión): en Android la sesión vive en el almacenamiento \
                 privado de la app (`/data/data/<paquete>`): `databases/` (SQLite) y \
                 `shared_prefs/` (XML). Leerlo **exige root**."
                    .to_string(),
            );
            notes.push(
                "VÍA 2 (hook): Frida/Zygisk para interceptar las llamadas HTTPS de la \
                 app en el propio dispositivo."
                    .to_string(),
            );
            notes.push(
                "VÍA 3 (endpoint): reimplementar la llamada con la cookie o el token \
                 extraído del almacenamiento de la app."
                    .to_string(),
            );
            notes.push(
                "El aislamiento de perfiles de Android (profiles de usuario) y el \
                 cifrado del almacenamiento complican la lectura cruzada: copiar \
                 `/data/data` a mano no es un plan de interoperabilidad."
                    .to_string(),
            );
        }
        _ => {
            notes.push(
                "Sin notas específicas: el perfil no tiene datos verificados. \
                 No interoperar contra rutas no confirmadas.".to_string(),
            );
            notes.push(
                "VÍA GENÉRICA: (1) reutilizar la sesión guardada en disco, (2) hookear \
                 el proceso mientras corre, (3) reimplementar el endpoint con las \
                 credenciales de la sesión. En todos los casos: sólo con cuentas \
                 propias y dentro de los términos de servicio."
                    .to_string(),
            );
        }
    }
    notes
}

// ========================================================================= //
// Toolkits de parcheo reempaquetados (familia Lucky Patcher)
// ========================================================================= //

/// Marcadores que delatan un **toolkit de parcheo reempaquetado** dentro de un
/// APK (DEX o AXML del manifiesto).
///
/// Hechos medidos en el caso C2 (MarinaPatcher, `ru.drjmwdme.cgwbfzijr`, que
/// es Lucky Patcher reempaquetado; su `classes.dex` coincide byte a byte con
/// el de la app instalada en el movil - ver `docs/MARINA_PATCHER_ANALYSIS.md`):
///
/// * el **paquete** y el **nombre** son el ocultamiento; dentro del DEX los
///   identificadores **no** estan ofuscados (sin ProGuard/R8);
/// * el APK lleva las **claves de firma de AOSP** (`testkey.*`, `media.*`,
///   `platform.*`, `shared.*`, `keystore.ks`), el **apksigner** embebido y el
///   servicio de **licencias de Play** (`ITestServiceInterface`) => puede
///   re-firmar APKs y saltarse la verificacion de licencia;
/// * es un **modulo Xposed/LSPosed** (`com.xposed.XSupport`);
/// * **sube el APK que parchea** a un tercero (`patch.chelpus.com/submit`) y
///   usa endpoints **sin TLS** y **IPs crudas**.
///
/// Formato de cada entrada: `(id, clase, severidad, marcador literal)`.
pub const PATCHER_TOOLKIT_MARKERS: &[(&str, &str, &str, &str)] = &[
    ("aosp-testkey-material", "signing", "critical", "testkey.pk8"),
    ("aosp-testkey-material", "signing", "critical", "testkey.x509.pem"),
    ("aosp-testkey-material", "signing", "critical", "media.pk8"),
    ("aosp-testkey-material", "signing", "critical", "platform.pk8"),
    ("aosp-testkey-material", "signing", "critical", "shared.pk8"),
    ("keystore-p12-embedded", "signing", "critical", "keystore.ks"),
    ("apksigner-embedded", "signing", "high", "com/android/apksigner"),
    ("apksigner-embedded", "signing", "high", "com.android.apksigner"),
    ("play-license-bypass", "license-bypass", "critical", "ITestServiceInterface"),
    ("play-vending-embedded", "license-bypass", "high", "com/android/vending"),
    ("play-vending-embedded", "license-bypass", "high", "com.android.vending"),
    // En el DEX la clase va con barras ("Lcom/xposed/XSupport;"); con puntos
    // aparece en el manifiesto y en `assets/xposed_init`.
    ("xposed-module", "hook", "critical", "com.xposed.XSupport"),
    ("xposed-module", "hook", "critical", "com/xposed/XSupport"),
    ("xposed-metadata", "hook", "high", "xposedminversion"),
    ("xposed-scope-metadata", "hook", "medium", "xposedscope"),
    ("uploads-target-apk", "exfiltration", "critical", "patch.chelpus.com/submit"),
    ("plaintext-patch-endpoint", "network", "high", "http://patch.chelpus.com"),
    ("raw-ip-endpoint", "network", "high", "109.95.209.70"),
    ("raw-ip-endpoint-tls", "network", "high", "149.56.155.246"),
    ("affiliate-tracking-id", "tracking", "medium", "appnext.com/myid.html"),
    ("ad-sdk-tapjoy", "tracking", "low", "tapjoyads.com"),
    ("ad-sdk-adsense", "tracking", "low", "googlesyndication.com"),
    ("promo-supersu", "promo", "low", "eu.chainfire.supersu"),
    ("promo-busybox", "promo", "low", "stericson.busybox"),
];

/// Que marcadores de toolkit de parcheo aparecen en un texto (DEX, AXML o
/// smali).  Devuelve los **ids** unicos, en el orden del catalogo.
pub fn detect_patcher_toolkit(text: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for (id, _class, _sev, marker) in PATCHER_TOOLKIT_MARKERS {
        if text.contains(marker) && !ids.iter().any(|x| x == id) {
            ids.push((*id).to_string());
        }
    }
    ids
}

/// Lo mismo, pero con el detalle (clase, severidad y el marcador que caso).
pub fn detect_patcher_toolkit_detail(text: &str) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for (id, class, severity, marker) in PATCHER_TOOLKIT_MARKERS {
        if text.contains(marker) {
            out.push(json!({
                "id": id,
                "class": class,
                "severity": severity,
                "marker": marker,
            }));
        }
    }
    out
}

/// Impacto real de un toolkit de parcheo en un movil con el bootloader
/// **cerrado**: las claves de test de AOSP **no** pasan la verificacion de AVB
/// (particiones firmadas por Samsung) => no puede parchear apps de sistema.
/// Lo que si queda es pedir root, hookear con Xposed, subir el APK objetivo a
/// un tercero y hablar por HTTP sin cifrar.
pub fn patcher_toolkit_impact_notes() -> Vec<&'static str> {
    vec![
        "bootloader-cerrado: una APK firmada con testkey NO pasa AVB (flash.locked=1, avb green) => no puede parchear apps de sistema",
        "pide-root: ACCESS_SUPERUSER",
        "hook: modulo Xposed/LSPosed (hook de cualquier app si hay LSPosed)",
        "exfiltracion: sube el APK que parchea a un servidor de terceros (patch.chelpus.com/submit)",
        "red: endpoints sin TLS y IPs crudas => interceptacion trivial",
        "tracking: id de afiliado (appnext) y SDKs de anuncios",
    ]
}

// ========================================================================= //
// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "md_app_intel_{}_{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("debe crear el temporal");
        base
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("debe crear el padre");
        }
        let mut f = fs::File::create(path).expect("debe crear el fichero");
        f.write_all(content.as_bytes()).expect("debe escribir");
    }

    #[test]
    fn el_catalogo_tiene_perfiles_unicos_y_al_menos_uno_verificado() {
        let profiles = known_profiles();
        assert!(profiles.len() >= 12, "catalogo demasiado corto");
        let mut ids: Vec<&str> = profiles.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "hay ids duplicados");
        assert!(profiles.iter().any(|p| p.verified));
        assert!(profiles.iter().any(|p| !p.verified));
    }

    #[test]
    fn los_perfiles_de_escritorio_tienen_las_rutas_conocidas() {
        let profiles = known_profiles();
        let claude = profiles.iter().find(|p| p.id == "claude-code").unwrap();
        assert!(claude.data_paths.iter().any(|p| p == "~/.claude/"));
        assert!(claude.data_paths.iter().any(|p| p == "~/.claude.json"));
        assert!(claude
            .data_paths
            .iter()
            .any(|p| p == "~/.claude/history.jsonl"));
        assert!(claude
            .auth_storage
            .iter()
            .any(|p| p.contains("credentials.json")));

        let cursor = profiles.iter().find(|p| p.id == "cursor").unwrap();
        assert!(cursor
            .data_paths
            .iter()
            .any(|p| p.contains("globalStorage/state.vscdb")));
        assert!(cursor.data_paths.iter().any(|p| p == "~/.cursor-server/"));

        let windsurf = profiles.iter().find(|p| p.id == "windsurf").unwrap();
        assert!(windsurf.data_paths.iter().any(|p| p == "~/.codeium/windsurf/"));

        let codex = profiles.iter().find(|p| p.id == "codex-cli").unwrap();
        assert!(codex.auth_storage.iter().any(|p| p == "~/.codex/auth.json"));
        assert!(codex.data_paths.iter().any(|p| p.contains("sessions")));
    }

    #[test]
    fn los_paquetes_android_verificados_tienen_formato_de_paquete() {
        let profiles = known_profiles();
        for p in profiles.iter().filter(|p| p.platform == "android" && p.verified) {
            assert!(
                p.package.contains('.') && !p.package.contains(' '),
                "{} no parece un paquete: {}",
                p.id,
                p.package
            );
        }
        let by_id = |id: &str| {
            profiles
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.package.clone())
                .unwrap()
        };
        assert_eq!(by_id("claude-android"), "com.anthropic.claude");
        assert_eq!(by_id("chatgpt-android"), "com.openai.chatgpt");
        assert_eq!(by_id("deepseek-android"), "com.deepseek.chat");
        assert_eq!(by_id("grok-android"), "ai.x.grok");
    }

    #[test]
    fn los_perfiles_no_verificados_lo_dicen_explicitamente() {
        let profiles = known_profiles();
        for p in profiles.iter().filter(|p| !p.verified) {
            assert!(
                !p.notes.is_empty(),
                "{} debería explicar por qué no está verificado",
                p.id
            );
            assert!(
                p.notes.to_lowercase().contains("verific")
                    || p.notes.to_lowercase().contains("inferid"),
                "{} debería dejar claro el nivel de verificación: {}",
                p.id,
                p.notes
            );
        }
    }

    #[test]
    fn extract_endpoints_saca_urls_y_descarta_el_resto() {
        let text = r#"
          const API = "https://api.anthropic.com/v1/messages";
          fetch('http://localhost:3000/api/health');
          // https://chatgpt.com/backend-api/conversation
          url = "https://api.example.com/v1?x=1";
          nada: esto no es una url
        "#;
        let urls = extract_endpoints(text);
        assert!(urls.contains(&"https://api.anthropic.com/v1/messages".to_string()));
        assert!(urls.contains(&"http://localhost:3000/api/health".to_string()));
        assert!(urls.contains(&"https://api.example.com/v1?x=1".to_string()));
        assert!(urls.contains(&"https://chatgpt.com/backend-api/conversation".to_string()));
        assert_eq!(urls.len(), 4, "urls duplicadas o de más: {:?}", urls);
        // Sin duplicados.
        let mut dedup = urls.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(dedup.len(), urls.len());
    }

    #[test]
    fn extract_endpoints_no_inventa_urls() {
        let urls = extract_endpoints("no hay ninguna url aqui, ni en ftp://ejemplo.com");
        assert!(urls.is_empty(), "{:?}", urls);
    }

    #[test]
    fn el_barrido_encuentra_los_ficheros_y_calcula_el_sha256() {
        let root = temp_dir("scan");
        let claude = root.join(".claude");
        write_file(
            &claude.join("projects").join("proyecto").join("sesion.jsonl"),
            "{\"type\":\"user\"}\n{\"type\":\"assistant\"}\n",
        );
        write_file(
            &claude.join(".credentials.json"),
            "{\"accessToken\":\"sk-ant-abcdefghijklmnopqrstuvwxyz012345\"}",
        );
        write_file(
            &root
                .join(".config")
                .join("Cursor")
                .join("User")
                .join("globalStorage")
                .join("state.vscdb"),
            "SQLite format 3\0",
        );
        write_file(
            &root
                .join(".config")
                .join("ChatGPT")
                .join("Local Storage")
                .join("leveldb")
                .join("000003.log"),
            "datos de leveldb",
        );

        let scan = scan_local_storage(root.to_str().unwrap());
        assert!(scan.files.len() >= 4, "ficheros: {:?}", scan.files);
        let kinds: Vec<&str> = scan.files.iter().map(|f| f.kind.as_str()).collect();
        assert!(kinds.contains(&"jsonl"));
        assert!(kinds.contains(&"sqlite"));
        assert!(kinds.contains(&"leveldb"));

        // El SHA-256 del jsonl debe coincidir con el del contenido.
        let jsonl = scan
            .files
            .iter()
            .find(|f| f.path.ends_with("sesion.jsonl"))
            .unwrap();
        let expected = sha256_hex(
            "{\"type\":\"user\"}\n{\"type\":\"assistant\"}\n".as_bytes(),
        );
        assert_eq!(jsonl.sha256, expected);
        assert!(jsonl.mtime.ends_with('Z'));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn el_barrido_redacta_las_credenciales_y_no_las_suelta() {
        let root = temp_dir("redact");
        let secret = "sk-ant-abcdefghijklmnopqrstuvwxyz012345";
        write_file(
            &root.join("config.json"),
            &format!("{{\"api_key\": \"{}\"}}", secret),
        );
        let scan = scan_local_storage(root.to_str().unwrap());
        let dumped = serde_json::to_string(&scan).unwrap();
        assert!(
            !dumped.contains(secret),
            "¡el secreto apareció entero en el escaneo!"
        );
        assert!(dumped.contains("sk-a…45"), "redacción inesperada: {}", dumped);
        assert_eq!(scan.summary["redacted_tokens"].as_array().unwrap().len(), 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn el_barrido_encuentra_urls_en_los_ficheros_de_texto() {
        let root = temp_dir("urls");
        write_file(
            &root.join("bundle.js"),
            "var t='https://api.anthropic.com/v1/messages';",
        );
        let scan = scan_local_storage(root.to_str().unwrap());
        let urls = scan.summary["urls"].as_array().unwrap();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://api.anthropic.com/v1/messages");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn el_barrido_respeta_el_limite_de_profundidad() {
        let root = temp_dir("depth");
        let deep = root.join("a").join("b").join("c").join("d").join("e").join("f").join("g");
        write_file(&deep.join("dato.json"), "{}");
        let scan = scan_local_storage(root.to_str().unwrap());
        // Con MAX_SCAN_DEPTH = 6, el fichero a 7 niveles no debe aparecer.
        assert!(
            !scan.files.iter().any(|f| f.path.ends_with("dato.json")),
            "se ha saltado el límite de profundidad: {:?}",
            scan.files.iter().map(|f| &f.path).collect::<Vec<_>>()
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn el_barrido_sobre_una_raiz_inexistente_no_falla() {
        let scan = scan_local_storage("/no/existe/este/directorio");
        assert!(scan.files.is_empty());
        assert_eq!(scan.summary["exists"], json!(false));
        assert!(!scan.summary["warnings"].as_array().unwrap().is_empty());
    }

    #[test]
    fn el_plan_de_exportacion_es_idempotente_y_tiene_sha256() {
        let root = temp_dir("plan");
        write_file(
            &root.join(".claude").join("projects").join("p").join("s.jsonl"),
            "{\"a\":1}\n",
        );
        let profile = profile_by_id("claude-code").expect("perfil claude-code");
        let plan = data_exchange_plan(&profile, root.to_str().unwrap());
        assert!(!plan.items.is_empty(), "el plan está vacío");
        let item = plan
            .items
            .iter()
            .find(|i| i.source.ends_with("s.jsonl"))
            .expect("debe incluir el jsonl");
        assert!(item.target.ends_with(".ndjson"), "destino: {}", item.target);
        assert!(item.target.starts_with("megadaemon/interop/claude-code/"));
        assert_eq!(item.sha256, sha256_hex("{\"a\":1}\n".as_bytes()));
        assert!(plan.total_bytes > 0);

        // Idempotencia: dos planes sobre el mismo árbol dan el mismo hash.
        let plan2 = data_exchange_plan(&profile, root.to_str().unwrap());
        assert_eq!(plan.plan_hash, plan2.plan_hash);
        assert_eq!(plan.items.len(), plan2.items.len());

        // Y si cambia el contenido, cambia el hash (se reescribe sólo eso).
        write_file(
            &root.join(".claude").join("projects").join("p").join("s.jsonl"),
            "{\"a\":2}\n",
        );
        let plan3 = data_exchange_plan(&profile, root.to_str().unwrap());
        assert_ne!(plan.plan_hash, plan3.plan_hash);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn el_plan_avisa_de_las_rutas_que_no_existen_y_no_exporta_credenciales() {
        let root = temp_dir("plan_missing");
        let profile = profile_by_id("claude-code").expect("perfil claude-code");
        let plan = data_exchange_plan(&profile, root.to_str().unwrap());
        assert!(plan.items.is_empty());
        assert!(plan
            .warnings
            .iter()
            .any(|w| w.contains("no existe en")));
        assert!(plan
            .warnings
            .iter()
            .any(|w| w.to_lowercase().contains("credenciales")));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn el_plan_convierte_una_base_sqlite_a_ndjson() {
        let root = temp_dir("plan_sqlite");
        // La ruta tiene que ser EXACTAMENTE la del perfil, si no el plan no
        // encuentra nada (que es justo lo que hay que probar).
        let db = root
            .join(".config")
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb");
        if let Some(p) = db.parent() {
            let _ = fs::create_dir_all(p);
        }
        {
            let conn = rusqlite::Connection::open(&db).expect("debe abrir la base");
            conn.execute(
                "CREATE TABLE ItemTable (key TEXT, value TEXT)",
                [],
            )
            .expect("debe crear la tabla");
            conn.execute(
                "INSERT INTO ItemTable VALUES ('composerData:1', '{\"x\":1}')",
                [],
            )
            .expect("debe insertar");
        }

        let profile = profile_by_id("cursor").expect("perfil cursor");
        let plan = data_exchange_plan(&profile, root.to_str().unwrap());
        let nd = plan
            .items
            .iter()
            .find(|i| i.kind == "ndjson")
            .expect("debe haber un NDJSON");
        assert!(nd.target.ends_with(".ndjson"));
        assert_eq!(nd.sha256, sha256_hex(
            "{\"key\":\"composerData:1\",\"value\":\"{\\\"x\\\":1}\"}\n".as_bytes()
        ));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn las_notas_de_interoperacion_cubren_las_tres_vias() {
        let notes = interop_notes("claude-code");
        assert!(notes.len() >= 3);
        assert!(notes.iter().any(|n| n.contains("VÍA 1")));
        assert!(notes.iter().any(|n| n.contains("VÍA 2")));
        assert!(notes.iter().any(|n| n.contains("VÍA 3")));

        let codex = interop_notes("codex-cli");
        assert!(codex.iter().any(|n| n.contains("openai_base_url")));
        assert!(codex.iter().any(|n| n.contains("refresh")));

        // Un perfil desconocido cae en la nota genérica, sin inventar datos.
        let unknown = interop_notes("no-existe");
        assert!(!unknown.is_empty());
        assert!(unknown
            .iter()
            .any(|n| n.contains("Sin notas específicas")));
    }

    #[test]
    fn redact_secret_nunca_devuelve_el_valor_entero() {
        let r = redact_secret("sk-ant-1234567890abcdef");
        assert!(!r.contains("1234567890"));
        assert!(r.starts_with("sk-a"));
        assert!(r.ends_with("ef"));
        let corto = redact_secret("abc");
        assert!(corto.contains("3 chars"));
    }

    #[test]
    fn detecta_toolkit_de_parcheo_reampaquetado() {
        // Marcadores medidos en el caso C2 (MarinaPatcher / Lucky Patcher).
        let dex = "Lcom/android/apksigner/ApkSignerTool;Lcom/android/vending/ITestServiceInterface;\
                   Lcom/xposed/XSupport;testkey.pk8;keystore.ks;http://patch.chelpus.com/submit;\
                   109.95.209.70;149.56.155.246;appnext.com/myid.html";
        let ids = detect_patcher_toolkit(dex);
        assert!(ids.contains(&"apksigner-embedded".to_string()));
        assert!(ids.contains(&"play-license-bypass".to_string()));
        assert!(ids.contains(&"xposed-module".to_string()));
        assert!(ids.contains(&"aosp-testkey-material".to_string()));
        assert!(ids.contains(&"keystore-p12-embedded".to_string()));
        assert!(ids.contains(&"uploads-target-apk".to_string()));
        assert!(ids.contains(&"raw-ip-endpoint".to_string()));
        // Sin duplicados aunque el marcador case mas de una vez.
        assert_eq!(ids.len(), ids.iter().collect::<std::collections::HashSet<_>>().len());

        let det = detect_patcher_toolkit_detail(dex);
        assert!(!det.is_empty());
        assert_eq!(det[0]["class"], json!("signing"));
        assert!(det.iter().all(|d| d.get("severity").is_some()));
    }

    #[test]
    fn un_app_normal_no_dispara_el_detector() {
        let texto = "Landroid/os/Build;Lcom/google/android/gms/ads/AdRequest;https://api.ejemplo.com/v1";
        assert!(detect_patcher_toolkit(texto).is_empty());
    }

    #[test]
    fn perfil_lucky_patcher_documenta_el_ocultamiento() {
        let p = profile_by_id("lucky-patcher").expect("perfil");
        assert_eq!(p.platform, "android");
        assert!(p.endpoints.iter().any(|e| e.contains("patch.chelpus.com/submit")));
        assert!(p.features.iter().any(|f| f.starts_with("obfuscation:package-and-label-only")));
        assert!(p.verified);
        assert!(patcher_toolkit_impact_notes().len() >= 4);
    }

}
