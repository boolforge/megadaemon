//! Esquema **ACP v1 vendorizado** (núcleo estable) — sin dependencias externas.
//!
//! # Por qué vendorizado y no como dependencia
//!
//! El esquema canónico de ACP vive en el crate `agent-client-protocol-schema`
//! (Apache-2.0) del repo `agentclientprotocol/agent-client-protocol`. Medido el
//! 2026-11-27:
//!
//! | dato | valor |
//! |---|---:|
//! | versión del crate | 1.22.0 |
//! | sha256 del `.crate` | `650e76dae0c8d3e772acb35184e6dd4defa0e15e130c96bf4c4586597f1f2d91` |
//! | líneas de `src/` | 54.732 |
//! | edition / rust-version | 2024 / 1.88.0 |
//! | dependencias | 8 (`anyhow`, `derive_more`, `indexmap`, `serde_with`, `strum`, `uuid`, `schemars`, `serde`/`serde_json`) |
//!
//! Traerlo como dependencia rompería los jobs de build si el registro no lo
//! sirve, y obligaría a adoptar **edition 2024**. Por eso aquí se **vendoriza**
//! el subconjunto del esquema que MegaDaemon necesita, reimplementado sobre
//! `serde` (que el crate ya tenía) y **sin ninguna dependencia nueva**.
//!
//! # Fidelidad al upstream
//!
//! Los nombres de campo, las etiquetas `serde` (`camelCase`, `tag = "type"`,
//! `snake_case`) y la forma de los JSON son los del crate upstream; se han
//! eliminado únicamente los atributos de `schemars` (generación de JSON Schema,
//! que no necesitamos) y los ayudantes de `serde_with` se han sustituido por
//! equivalentes con `serde` a secas.
//!
//! # Qué NO se vendoriza (y por qué)
//!
//! * Las variantes `unstable_*` (`PromptSuggestionsUpdate`, `CursorUpdate`, …)
//!   y las capacidades nuevas (`session.notices`, `session.compaction`,
//!   `subagents`): el crate las marca como no estables y el registro oficial
//!   **no las prueba**.
//! * `elicitation`, `plan`, `proxy`, `mcp` completos: 3.000-7.000 líneas que
//!   MegaDaemon todavía no habla; se añaden cuando el transporte las use.
//!
//! # Licencia
//!
//! El código de este módulo es una adaptación de
//! `agent-client-protocol-schema` 1.22.0, **Apache-2.0**; la atribución
//! completa está en `docs/ACP_VENDOR_LICENSE.md` y `docs/ACP_VENDOR.md`.
//! Los avisos de licencia originales van en `docs/ACP_VENDOR_LICENSE.md`.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Versión del protocolo que este módulo implementa.
///
/// MEDIDO (registro ACP oficial, 2026-11-26): de los 42 agentes probados,
/// 41 inicializan y **los 41 usan la versión 1**.
pub const VERSION_PROTOCOLO: u16 = 1;

/// Versión del crate upstream del que se derivó este módulo.
pub const ESQUEMA_CANONICO: &str = "1.22.0";

/// sha256 del `.crate` upstream (medido el 2026-11-27).
pub const ESQUEMA_CANONICO_SHA256: &str =
    "650e76dae0c8d3e772acb35184e6dd4defa0e15e130c96bf4c4586597f1f2d91";

/// Metadatos libres (`_meta`), que ACP reserva para extensiones.
///
/// Se usa `BTreeMap` y no `HashMap` para que la serialización sea
/// **determinista**: el mismo estado produce siempre el mismo JSON.
pub type Meta = BTreeMap<String, Value>;

// ---------------------------------------------------------------------------
// Utilidades de deserialización (sustituyen a `serde_with`)
// ---------------------------------------------------------------------------

/// Deserializa con valor por defecto si el campo viene mal formado.
///
/// Equivalente a `serde_with::DefaultOnError` combinado con `#[serde(default)]`.
fn default_on_error<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    // MEDIDO (2026-11-27): la funcion se llama "default_on_error" pero NO
    // se tragaba el error (`?` lo propagaba): un campo ROTO del cliente
    // (`"annotations": 42`) tumbaba la sesion entera en vez de caer al
    // valor por defecto.
    match Option::<T>::deserialize(d) {
        Ok(Some(v)) => Ok(v),
        Ok(None) => Ok(T::default()),
        Err(_) => Ok(T::default()),
    }
}

// ---------------------------------------------------------------------------
// Identificadores
// ---------------------------------------------------------------------------

/// Identificador de petición JSON-RPC: número o cadena.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// Identificador numérico.
    Number(i64),
    /// Identificador textual.
    Text(String),
}

impl RequestId {
    /// Identificador a partir de un entero.
    pub fn from_number(n: i64) -> Self {
        RequestId::Number(n)
    }

    /// Identificador a partir de una cadena.
    pub fn from_text(s: impl Into<String>) -> Self {
        RequestId::Text(s.into())
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestId::Number(n) => write!(f, "{n}"),
            RequestId::Text(s) => f.write_str(s),
        }
    }
}

/// Identificador de sesión.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub String);

impl SessionId {
    /// Construye el identificador a partir de un texto.
    pub fn new(s: impl Into<String>) -> Self {
        SessionId(s.into())
    }

    /// El identificador como texto.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Modo de sesión (p.ej. el modo de permisos o el "modo agente").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    /// Sesión normal.
    Default,
    /// Modo propio, con identificador y nombre.
    Custom {
        /// Identificador del modo.
        id: String,
        /// Nombre legible.
        name: String,
        /// Descripción opcional.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Contenido
// ---------------------------------------------------------------------------

/// Bloque de contenido: texto, imagen, audio o recurso.
///
/// Compatible con MCP, de modo que la salida de una herramienta se puede
/// reenviar sin transformar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Texto plano o Markdown.
    Text(TextContent),
    /// Imagen.
    Image(ImageContent),
    /// Audio.
    Audio(AudioContent),
    /// Enlace a un recurso accesible por el agente.
    ResourceLink(ResourceLink),
    /// Recurso embebido en el mensaje.
    Resource(EmbeddedResource),
}

impl From<String> for ContentBlock {
    fn from(text: String) -> Self {
        ContentBlock::Text(TextContent::new(text))
    }
}

impl From<&str> for ContentBlock {
    fn from(text: &str) -> Self {
        ContentBlock::Text(TextContent::new(text))
    }
}

/// Texto del modelo o del usuario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextContent {
    /// Anotaciones de presentación.
    ///
    /// MEDIDO (2026-11-27): sin `skip_serializing_if` salía
    /// `"annotations":null` en el JSON ⇒ el texto no era
    /// `{"text":"x"}` (lo que exige el protocolo).
    #[serde(
        default,
        deserialize_with = "default_on_error",
        skip_serializing_if = "Option::is_none"
    )]
    pub annotations: Option<Annotations>,
    /// El texto.
    pub text: String,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

impl TextContent {
    /// Construye un bloque de texto.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            annotations: None,
            text: text.into(),
            meta: None,
        }
    }
}

/// Imagen para contexto visual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageContent {
    /// Tipo MIME.
    pub mime_type: String,
    /// Datos en base64.
    pub data: String,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Audio para transcripción o análisis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioContent {
    /// Tipo MIME.
    pub mime_type: String,
    /// Datos en base64.
    pub data: String,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Referencia a un recurso.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLink {
    /// URI del recurso.
    pub uri: String,
    /// Tipo MIME del recurso.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Nombre del recurso.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Recurso con el contenido embebido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedResource {
    /// El recurso.
    pub resource: ResourceContents,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Contenido de un recurso.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContents {
    /// URI del recurso.
    pub uri: String,
    /// Tipo MIME.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Texto del recurso.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Recurso binario en base64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

/// Anotaciones de presentación de un bloque de texto.
///
/// MEDIDO (2026-11-27): `priority` es un `Option<f64>` (0..1, como en el
/// upstream) y `f64` NO implementa `Eq` => el derive `Eq` rompia la
/// compilacion.  Se queda `PartialEq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotations {
    /// Audiencia: `user` o `assistant`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audience: Option<Vec<String>>,
    /// Prioridad: de 0 a 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
}

// ---------------------------------------------------------------------------
// Tool calls
// ---------------------------------------------------------------------------

/// Llamada a una herramienta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    /// Identificador de la llamada.
    pub tool_call_id: String,
    /// Título legible.
    pub title: String,
    /// Estado actual.
    pub kind: ToolKind,
    /// Estado de ejecución.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ToolCallStatus>,
    /// Ubicación en el código, si la herramienta la aporta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<ToolCallLocation>>,
    /// Entrada cruda de la herramienta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_input: Option<Value>,
    /// Salida cruda de la herramienta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<Value>,
    /// Contenido legible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ToolCallContent>>,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Clase de herramienta.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    /// Lee recursos.
    Read,
    /// Edita recursos.
    Edit,
    /// Borra recursos.
    Delete,
    /// Mueve recursos.
    Move,
    /// Busca recursos.
    Search,
    /// Ejecuta un comando.
    Execute,
    /// Razonamiento interno del agente.
    Think,
    /// Otro.
    Other,
    /// Herramienta de servidor MCP.
    Mcp,
}

/// Estado de una llamada a herramienta.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    /// En cola.
    Pending,
    /// En curso.
    InProgress,
    /// Terminada con éxito.
    Completed,
    /// Terminada con error.
    Failed,
}

/// Contenido de una llamada a herramienta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolCallContent {
    /// Texto.
    Content(ContentBlock),
    /// Diff.
    Diff(DiffContent),
    /// Recurso.
    Resource(ResourceContents),
}

/// Diff unificado entre dos versiones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffContent {
    /// Ruta del fichero.
    pub path: String,
    /// Texto antes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_text: Option<String>,
    /// Texto después.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_text: Option<String>,
}

/// Posición en el código.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallLocation {
    /// Ruta del fichero.
    pub path: String,
    /// Línea (1-based).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

/// Actualización de una llamada a herramienta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallUpdate {
    /// Identificador de la llamada.
    pub tool_call_id: String,
    /// Estado nuevo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ToolCallStatus>,
    /// Título nuevo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Entrada cruda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_input: Option<Value>,
    /// Salida cruda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<Value>,
    /// Contenido legible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ToolCallContent>>,
    /// Ubicaciones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<ToolCallLocation>>,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

// ---------------------------------------------------------------------------
// Actualizaciones de sesión
// ---------------------------------------------------------------------------

/// Actualización que el agente envía al cliente durante una sesión.
///
/// Se vendorizan las variantes **estables** del upstream; las `unstable_*` y las
/// de capacidades nuevas no se incluyen a propósito.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// MEDIDO (2026-11-27): las VARIANTES van en snake_case
// ("agent_message_chunk") y los CAMPOS en camelCase.  El `rename_all` del
// enum afecta a las VARIANTES, asi que aqui va "snake_case"; el camelCase
// de los campos lo pone cada struct.  Ver docs/ACP_VENDOR.md:100 y
// docs/RE_ACP.md:252.
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
pub enum SessionUpdate {
    /// Fragmento del mensaje del usuario.
    UserMessageChunk(ContentChunk),
    /// Fragmento de la respuesta del agente.
    AgentMessageChunk(ContentChunk),
    /// Fragmento del razonamiento interno del agente.
    AgentThoughtChunk(ContentChunk),
    /// Nueva llamada a herramienta.
    ToolCall(ToolCall),
    /// Actualización de una llamada a herramienta.
    ToolCallUpdate(ToolCallUpdate),
    /// Comandos disponibles.
    AvailableCommandsUpdate(AvailableCommandsUpdate),
    /// El modo de la sesión ha cambiado.
    CurrentModeUpdate(CurrentModeUpdate),
}

/// Fragmento de contenido en streaming.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentChunk {
    /// Contenido.
    pub content: ContentBlock,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Comandos que el agente pone a disposición.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableCommandsUpdate {
    /// Lista de comandos.
    pub available_commands: Vec<AvailableCommand>,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Un comando disponible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableCommand {
    /// Nombre del comando.
    pub name: String,
    /// Descripción.
    pub description: String,
    /// Entrada que espera.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<AvailableCommandInput>,
}

/// Entrada de un comando disponible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableCommandInput {
    /// Si la entrada es texto libre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// El modo actual de la sesión ha cambiado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentModeUpdate {
    /// Identificador del modo actual.
    pub current_mode_id: String,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Motivo por el que termina un turno.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Fin natural del turno.
    EndTurn,
    /// Tope de tokens alcanzado.
    MaxTokens,
    /// Tope de turnos alcanzado.
    MaxTurnRequests,
    /// Cancelado por el cliente.
    Cancelled,
    /// El cliente pidió el refinamiento.
    Refusal,
}

// ---------------------------------------------------------------------------
// Capacidades e inicialización
// ---------------------------------------------------------------------------

/// Lo que el cliente puede hacer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    /// Capacidades de prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_capabilities: Option<PromptCapabilities>,
    /// Si el cliente soporta `session/load`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_session: Option<bool>,
    /// Si el cliente soporta `session/list`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_sessions: Option<bool>,
}

/// Capacidades de prompt del cliente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCapabilities {
    /// Si el cliente acepta contenido de imagen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<bool>,
    /// Si el cliente acepta contenido de audio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<bool>,
    /// Contexto embebido.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded_context: Option<bool>,
}

/// Lo que el agente sabe hacer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    /// Si el agente carga sesiones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_session: Option<bool>,
    /// Si el agente permite cancelar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_capabilities: Option<PromptCapabilities>,
}

/// Petición `initialize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    /// Versión del protocolo que pide el cliente.
    pub protocol_version: u16,
    /// Capacidades del cliente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_capabilities: Option<ClientCapabilities>,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Respuesta `initialize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    /// Versión del protocolo que responde el agente.
    pub protocol_version: u16,
    /// Capacidades del agente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_capabilities: Option<AgentCapabilities>,
    /// Información del agente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_info: Option<AgentInfo>,
    /// Métodos de autenticación admitidos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_methods: Option<Vec<AuthMethod>>,
}

/// Identidad del agente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    /// Nombre.
    pub name: String,
    /// Versión del agente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Método de autenticación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthMethod {
    /// Token/API key.
    ApiKey,
    /// Credenciales de terminal.
    Terminal,
    /// OAuth.
    OAuth,
}

// ---------------------------------------------------------------------------
// Sesión
// ---------------------------------------------------------------------------

/// Petición `session/new`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionRequest {
    /// Directorio de trabajo.
    pub cwd: String,
    /// Identificadores de servidores MCP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<McpServer>>,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Respuesta `session/new`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionResponse {
    /// Identificador de la sesión creada.
    pub session_id: SessionId,
    /// Modos disponibles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modes: Option<SessionModeState>,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Estado de modos de una sesión.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionModeState {
    /// Modo actual.
    pub current_mode_id: String,
    /// Modos disponibles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_modes: Option<Vec<SessionMode>>,
}

/// Petición `session/prompt`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptRequest {
    /// Identificador de la sesión.
    pub session_id: SessionId,
    /// Bloques del mensaje.
    pub prompt: Vec<ContentBlock>,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Respuesta `session/prompt`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptResponse {
    /// Motivo de fin de turno.
    pub stop_reason: StopReason,
    /// Metadatos libres.
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Servidor MCP disponible para la sesión.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum McpServer {
    /// Servidor local por línea de comandos.
    Stdio {
        /// Nombre.
        name: String,
        /// Comando.
        command: String,
        /// Argumentos.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<Vec<String>>,
        /// Variables de entorno.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        env: Option<Vec<EnvVar>>,
    },
    /// Servidor HTTP.
    Http {
        /// Nombre.
        name: String,
        /// URL.
        url: String,
        /// Cabeceras.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        headers: Option<Vec<HttpHeader>>,
    },
}

/// Variable de entorno para un servidor MCP por proceso.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    /// Nombre.
    pub name: String,
    /// Valor.
    pub value: String,
}

/// Cabecera HTTP para un servidor MCP.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpHeader {
    /// Nombre.
    pub name: String,
    /// Valor.
    pub value: String,
}

// ---------------------------------------------------------------------------
// Errores
// ---------------------------------------------------------------------------

/// Error de la capa ACP.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    /// Código.
    pub code: ErrorCode,
    /// Mensaje legible.
    pub message: String,
    /// Datos extra.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Códigos de error de ACP.
///
/// Se replican los del JSON-RPC más los específicos del protocolo.
// MEDIDO (2026-11-27): el derive de `Serialize`/`Deserialize` emitia el
// NOMBRE del variant (`"methodNotFound"`); pero el JSON-RPC lleva el
// **NUMERO** del codigo (`-32601`).  Se implementan a mano.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorCode {
    /// JSON inválido.
    ParseError,
    /// Petición inválida.
    InvalidRequest,
    /// Método inexistente.
    MethodNotFound,
    /// Parámetros inválidos.
    InvalidParams,
    /// Error interno.
    InternalError,
    /// Petición cancelada.
    RequestCancelled,
    /// Autenticación requerida.
    AuthRequired,
    /// Recurso no encontrado.
    ResourceNotFound,
}

impl ErrorCode {
    /// El número del código, tal y como va en el JSON-RPC.
    pub fn as_i64(self) -> i64 {
        match self {
            ErrorCode::ParseError => -32700,
            ErrorCode::InvalidRequest => -32600,
            ErrorCode::MethodNotFound => -32601,
            ErrorCode::InvalidParams => -32602,
            ErrorCode::InternalError => -32603,
            ErrorCode::RequestCancelled => -32800,
            ErrorCode::AuthRequired => -32000,
            ErrorCode::ResourceNotFound => -32002,
        }
    }

    /// Construye el código a partir de su número.
    pub fn from_i64(n: i64) -> Option<ErrorCode> {
        Some(match n {
            -32700 => ErrorCode::ParseError,
            -32600 => ErrorCode::InvalidRequest,
            -32601 => ErrorCode::MethodNotFound,
            -32602 => ErrorCode::InvalidParams,
            -32603 => ErrorCode::InternalError,
            -32800 => ErrorCode::RequestCancelled,
            -32000 => ErrorCode::AuthRequired,
            -32002 => ErrorCode::ResourceNotFound,
            _ => return None,
        })
    }
}

impl Serialize for ErrorCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(self.as_i64())
    }
}

impl<'de> Deserialize<'de> for ErrorCode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let n = i64::deserialize(deserializer)?;
        ErrorCode::from_i64(n)
            .ok_or_else(|| serde::de::Error::custom(format!("codigo ACP desconocido: {}", n)))
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ErrorCode::ParseError => "Parse error",
            ErrorCode::InvalidRequest => "Invalid request",
            ErrorCode::MethodNotFound => "Method not found",
            ErrorCode::InvalidParams => "Invalid params",
            ErrorCode::InternalError => "Internal error",
            ErrorCode::RequestCancelled => "Request cancelled",
            ErrorCode::AuthRequired => "Authentication required",
            ErrorCode::ResourceNotFound => "Resource not found",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_content_block_usa_la_etiqueta_type() {
        let j = serde_json::to_string(&ContentBlock::from("hola")).unwrap();
        assert!(j.contains("\"type\":\"text\""), "{j}");
        assert!(j.contains("\"text\":\"hola\""), "{j}");
    }

    #[test]
    fn el_session_update_usa_la_etiqueta_session_update() {
        let u = SessionUpdate::AgentMessageChunk(ContentChunk {
            content: ContentBlock::from("hola"),
            meta: None,
        });
        let j = serde_json::to_string(&u).unwrap();
        assert!(j.contains("\"sessionUpdate\":\"agent_message_chunk\""), "{j}");
    }

    #[test]
    fn el_texto_usa_camel_case() {
        let t = TextContent::new("x");
        let j = serde_json::to_string(&t).unwrap();
        assert!(!j.contains("annotations"), "{j}");
        assert_eq!(j, "{\"text\":\"x\"}");
    }

    #[test]
    fn los_codigos_de_error_coinciden_con_json_rpc() {
        assert_eq!(ErrorCode::MethodNotFound.as_i64(), -32601);
        assert_eq!(ErrorCode::RequestCancelled.as_i64(), -32800);
        assert_eq!(
            ErrorCode::from_i64(-32601),
            Some(ErrorCode::MethodNotFound)
        );
        assert_eq!(ErrorCode::from_i64(1), None);
    }

    #[test]
    fn el_texto_se_deserializa_sin_annotations() {
        let t: TextContent = serde_json::from_str("{\"text\":\"hola\"}").unwrap();
        assert_eq!(t.text, "hola");
        assert!(t.annotations.is_none());
    }

    #[test]
    fn las_annotations_rotoas_caen_a_none() {
        let t: TextContent =
            serde_json::from_str("{\"text\":\"hola\",\"annotations\":42}").unwrap();
        assert!(t.annotations.is_none());
    }

    #[test]
    fn el_meta_es_determinista() {
        let mut m = Meta::new();
        m.insert("b".into(), Value::from(2));
        m.insert("a".into(), Value::from(1));
        let j = serde_json::to_string(&m).unwrap();
        assert_eq!(j, "{\"a\":1,\"b\":2}");
    }

    #[test]
    fn el_id_de_peticion_soporta_numero_y_texto() {
        let n: RequestId = serde_json::from_str("7").unwrap();
        assert_eq!(n, RequestId::Number(7));
        let t: RequestId = serde_json::from_str("\"abc\"").unwrap();
        assert_eq!(t, RequestId::Text("abc".into()));
        assert_eq!(RequestId::Number(7).to_string(), "7");
    }

    #[test]
    fn la_version_es_la_que_mide_el_registro() {
        assert_eq!(VERSION_PROTOCOLO, 1);
        assert_eq!(ESQUEMA_CANONICO, "1.22.0");
        assert_eq!(ESQUEMA_CANONICO_SHA256.len(), 64);
    }
}
