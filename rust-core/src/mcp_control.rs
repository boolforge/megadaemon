//! Control de **servidores y tools MCP** (opt-in mutuo).
//!
//! ## Qué capacidad se implementa
//!
//! Encender y apagar los servidores MCP que conoce el daemon y, dentro de
//! cada servidor, encender y apagar **tools** concretas.  Tres operaciones:
//! listar servidores, encender/apagar un servidor y encender/apagar una tool.
//!
//! ## Fuente MEDIDA (se documenta, no se transcribe)
//!
//! * `docs/RE_ACP.md:345` — §6.1, fila 19: la capacidad `cognition.ai/mcp`
//!   expone `cognition.ai/mcp/{listServers,toggleServer,toggleTool}` y es
//!   **opt-in mutuo**.
//! * `docs/RE_ACP.md:319-321` — §6: las capacidades `cognition.ai/*` van en
//!   `initialize.params._meta` y son **opt-in**: "el agente sólo anuncia la
//!   capacidad correspondiente si el cliente la pide" (y el cliente, por su
//!   parte, sólo pide lo que sabe hacer ⇒ de ahí el "mutuo").
//! * `docs/RE_ACP_COMPARADO.md:305` — §5: Devin y los demás productos tienen
//!   extensiones propias (`cognition.ai/*`); `docs/RE_ACP_COMPARADO.md:358-362`
//!   — §6.5: las extensiones van en `_meta` con prefijo de producto ⇒ las
//!   **nuestras** van en el namespace `mdaemon`
//!   (`rust-core/src/protocol_acp.rs:73`).
//!
//! ## La implementación es NUESTRA
//!
//! Procedimiento **AST → Rust** del repo (`docs/EXTENSIONES_PROPIETARIAS.md`
//! §4, líneas 51-78): OUR Rust, OUR nombres, OUR estructura.  La equivalencia
//! que se busca es de **comportamiento observable** (mismas entradas
//! aceptadas/rechazadas, misma salida, mismos efectos secundarios), NO el
//! mismo texto ni los mismos nombres.
//!
//! ## Dónde cuelga (el registro REAL)
//!
//! El catálogo y el dispatch reales de las tools MCP están en
//! `rust-core/src/mcp_ext.rs` (definiciones en la línea 15, dispatch en la
//! línea 1180).  Este módulo **no inventa otro registry**: el acceso al
//! registro real va detrás del rasgo [`McpRegistry`], que el root implementa
//! sobre el `mcp_config.json` de verdad.
//!
//! Lo que ya está medido en el repo sobre ese fichero:
//!
//! * `rust-core/src/service_bus.rs:1568` — los servidores MCP que el daemon
//!   configura viven en la clave `mcpServers` del `mcp_config.json`.
//! * `rust-core/src/service_bus.rs:1564-1566` — la whitelist de tools por
//!   servidor (`enabled_tools`) **ya se escribe** en la entrada del servidor
//!   ⇒ el "toggle de tool" tiene dónde persistirse de verdad.
//!
//! ## Integración posterior (la hace el root, NO este fichero)
//!
//! 1. `mcp_ext::tool_definitions()` ⇒ concatenar `mcp_control::tool_definitions()`.
//! 2. `mcp_ext::dispatch()` ⇒ `if let Some(r) = crate::mcp_control::dispatch_mcp(name, args, &mut registro, &declaracion) { return Some(r); }`.
//! 3. `protocol_acp::extensiones_propias()` ⇒ registrar `mcp_control::extensiones()`
//!    con el namespace `mdaemon`.
//! 4. Implementar `McpRegistry` sobre el `mcp_config.json` real (leer
//!    `mcpServers`, escribir `enabled` + `enabled_tools`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

/// Cómo se alcanza un servidor MCP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServerTarget {
    /// Servidor remoto (HTTP/SSE): se guarda su URL.
    Url {
        /// La URL del servidor.
        url: String,
    },
    /// Servidor local: se guarda el comando y sus argumentos.
    Command {
        /// El programa a ejecutar.
        program: String,
        /// Sus argumentos.
        #[serde(default)]
        args: Vec<String>,
    },
}

impl ServerTarget {
    /// Descripción corta (para el listado y los logs).
    pub fn describe(&self) -> String {
        match self {
            ServerTarget::Url { url } => url.clone(),
            ServerTarget::Command { program, args } => {
                if args.is_empty() {
                    program.clone()
                } else {
                    format!("{} {}", program, args.join(" "))
                }
            }
        }
    }
}

/// Una tool dentro de un servidor MCP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTool {
    /// El nombre con el que el cliente la invoca.
    pub name: String,
    /// ¿Está habilitada?
    pub enabled: bool,
}

/// Un servidor MCP que conoce el daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServer {
    /// Nombre único del servidor.
    pub name: String,
    /// Cómo se alcanza.
    pub target: ServerTarget,
    /// ¿El servidor está encendido?
    pub enabled: bool,
    /// Sus tools, con su estado individual.
    pub tools: Vec<McpTool>,
}

impl McpServer {
    /// Busca una de sus tools por nombre.
    pub fn tool(&self, name: &str) -> Option<&McpTool> {
        self.tools.iter().find(|t| t.name == name)
    }
}

/// El estado completo del registro MCP.
///
/// Un `BTreeMap` por nombre ⇒ el listado sale siempre en el mismo orden
/// (salida comparable entre llamadas, que es lo que se testea).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpState {
    /// Los servidores, por nombre.
    #[serde(default)]
    pub servers: BTreeMap<String, McpServer>,
}

impl McpState {
    /// Estado vacío.
    pub fn new() -> Self {
        Self {
            servers: BTreeMap::new(),
        }
    }

    /// Inserta (o sustituye) un servidor.
    pub fn insert(&mut self, server: McpServer) {
        self.servers.insert(server.name.clone(), server);
    }

    /// Busca un servidor por nombre.
    pub fn server(&self, name: &str) -> Option<&McpServer> {
        self.servers.get(name)
    }
}

/// Las capacidades MCP: las tres operaciones medidas.
///
/// Los identificadores son **nuestros** (el namespace `mdaemon` es nuestro);
/// el del cliente va en su propia declaración.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum McpCapability {
    /// Listar los servidores y el estado de sus tools.
    ListServers,
    /// Encender o apagar un servidor.
    ToggleServer,
    /// Encender o apagar una tool de un servidor.
    ToggleTool,
}

impl McpCapability {
    /// Identificador estable (el que viaja en la declaración del cliente).
    pub fn id(&self) -> &'static str {
        match self {
            McpCapability::ListServers => "list-servers",
            McpCapability::ToggleServer => "toggle-server",
            McpCapability::ToggleTool => "toggle-tool",
        }
    }

    /// Todas las capacidades, en orden estable.
    pub fn all() -> [McpCapability; 3] {
        [
            McpCapability::ListServers,
            McpCapability::ToggleServer,
            McpCapability::ToggleTool,
        ]
    }

    /// Parsea un identificador.
    pub fn from_id(id: &str) -> Option<McpCapability> {
        McpCapability::all().into_iter().find(|c| c.id() == id)
    }
}

/// Lo que el cliente ha declarado que quiere (el **opt-in mutuo**).
///
/// Sin declaración no hay nada que anunciar ni nada que aceptar: el daemon
/// no inventa capacidades que el cliente no pidió.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declaration {
    /// Las capacidades declaradas (sin repetir, ordenadas).
    #[serde(default)]
    pub declared: Vec<McpCapability>,
}

impl Declaration {
    /// Declaración vacía (el cliente no pidió nada).
    pub fn none() -> Self {
        Self {
            declared: Vec::new(),
        }
    }

    /// Declaración con un conjunto de capacidades.
    pub fn of(caps: &[McpCapability]) -> Self {
        let mut declared: Vec<McpCapability> = Vec::new();
        for c in caps {
            if !declared.contains(c) {
                declared.push(*c);
            }
        }
        declared.sort();
        Self { declared }
    }

    /// ¿El cliente ha declarado esta capacidad?
    pub fn contains(&self, cap: McpCapability) -> bool {
        self.declared.contains(&cap)
    }

    /// Lo que se anuncia: exactamente lo declarado, ni una capacidad más.
    pub fn announced(&self) -> &[McpCapability] {
        &self.declared
    }

    /// Parsea la declaración que llega del cliente.
    ///
    /// Acepta una lista (`["list-servers", …]`) o un objeto con la clave
    /// `capabilities`.  Un identificador que no es nuestro es un **error
    /// tipado**: se dice, no se ignora en silencio (un cliente que declara
    /// cosas que no existen está roto y hay que decírselo).
    pub fn from_json(value: &Value) -> Result<Self, McpControlError> {
        let items: Vec<&Value> = match value {
            Value::Array(a) => a.iter().collect(),
            Value::Object(o) => match o.get("capabilities") {
                Some(Value::Array(a)) => a.iter().collect(),
                Some(_) => {
                    return Err(McpControlError::MalformedDeclaration {
                        reason: "'capabilities' no es una lista".to_string(),
                    })
                }
                None => {
                    return Err(McpControlError::MalformedDeclaration {
                        reason: "falta la lista 'capabilities'".to_string(),
                    })
                }
            },
            _ => {
                return Err(McpControlError::MalformedDeclaration {
                    reason: "la declaración no es una lista ni un objeto".to_string(),
                })
            }
        };
        let mut caps: Vec<McpCapability> = Vec::new();
        for item in items {
            let id = item
                .as_str()
                .ok_or_else(|| McpControlError::MalformedDeclaration {
                    reason: "una capacidad declarada no es texto".to_string(),
                })?;
            let cap = McpCapability::from_id(id).ok_or_else(|| {
                McpControlError::MalformedDeclaration {
                    reason: format!("capacidad desconocida: {id}"),
                }
            })?;
            caps.push(cap);
        }
        Ok(Declaration::of(&caps))
    }
}

/// Los errores TIPADOS del control MCP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpControlError {
    /// Ese servidor no existe en el registro.
    UnknownServer {
        /// El nombre pedido.
        name: String,
    },
    /// Esa tool no existe en ese servidor.
    UnknownTool {
        /// El servidor pedido.
        server: String,
        /// La tool pedida.
        tool: String,
    },
    /// El servidor está apagado: no se puede cambiar el estado de sus tools.
    ServerDisabled {
        /// El servidor apagado.
        server: String,
    },
    /// El cliente no declaró esa capacidad (opt-in mutuo).
    CapabilityNotDeclared {
        /// El identificador de la capacidad que faltaba.
        capability: &'static str,
    },
    /// La declaración del cliente está mal formada.
    MalformedDeclaration {
        /// Por qué.
        reason: String,
    },
    /// El estado (el `mcp_config.json` real) está mal formado.
    MalformedState {
        /// Por qué.
        reason: String,
    },
    /// El registro real no se pudo leer o escribir.
    Registry {
        /// Por qué.
        reason: String,
    },
}

impl fmt::Display for McpControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpControlError::UnknownServer { name } => {
                write!(f, "servidor MCP desconocido: {name}")
            }
            McpControlError::UnknownTool { server, tool } => {
                write!(f, "tool MCP desconocida: {tool} (servidor {server})")
            }
            McpControlError::ServerDisabled { server } => {
                write!(f, "el servidor MCP {server} está apagado")
            }
            McpControlError::CapabilityNotDeclared { capability } => {
                write!(f, "el cliente no declaró la capacidad {capability}")
            }
            McpControlError::MalformedDeclaration { reason } => {
                write!(f, "declaración del cliente mal formada: {reason}")
            }
            McpControlError::MalformedState { reason } => {
                write!(f, "estado MCP mal formado: {reason}")
            }
            McpControlError::Registry { reason } => {
                write!(f, "no se pudo tocar el registro MCP: {reason}")
            }
        }
    }
}

impl std::error::Error for McpControlError {}

/// La **decisión**: qué se devuelve o qué hay que cambiar.
///
/// Es una función pura del estado: dice QUÉ cambiar, no lo cambia.  Aplicarla
/// es otra etapa ([`apply`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// El listado de servidores (no muta nada).
    Listing(Vec<McpServer>),
    /// Encender o apagar un servidor.
    Server {
        /// El servidor afectado.
        name: String,
        /// El estado pedido.
        enabled: bool,
        /// ¿De verdad cambia algo? (si ya estaba así, no: es idempotencia).
        changed: bool,
    },
    /// Encender o apagar una tool de un servidor.
    Tool {
        /// El servidor afectado.
        server: String,
        /// La tool afectada.
        tool: String,
        /// El estado pedido.
        enabled: bool,
        /// ¿De verdad cambia algo?
        changed: bool,
    },
}

impl Decision {
    /// ¿Esta decisión muta el estado?
    pub fn changed(&self) -> bool {
        match self {
            Decision::Listing(_) => false,
            Decision::Server { changed, .. } => *changed,
            Decision::Tool { changed, .. } => *changed,
        }
    }

    /// La decisión en JSON (la forma que ve el cliente).
    pub fn to_json(&self) -> Value {
        match self {
            Decision::Listing(servers) => json!({
                "servers": servers.iter().map(server_to_json).collect::<Vec<Value>>(),
                "count": servers.len(),
            }),
            Decision::Server {
                name,
                enabled,
                changed,
            } => json!({
                "server": name,
                "enabled": enabled,
                "changed": changed,
            }),
            Decision::Tool {
                server,
                tool,
                enabled,
                changed,
            } => json!({
                "server": server,
                "tool": tool,
                "enabled": enabled,
                "changed": changed,
            }),
        }
    }
}

/// Un servidor en JSON (la forma que ve el cliente).
pub fn server_to_json(server: &McpServer) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("name".to_string(), json!(server.name));
    match &server.target {
        ServerTarget::Url { url } => {
            out.insert("url".to_string(), json!(url));
        }
        ServerTarget::Command { program, args } => {
            out.insert("command".to_string(), json!(program));
            if !args.is_empty() {
                out.insert("args".to_string(), json!(args));
            }
        }
    }
    out.insert("enabled".to_string(), json!(server.enabled));
    out.insert(
        "tools".to_string(),
        Value::Array(
            server
                .tools
                .iter()
                .map(|t| json!({ "name": t.name, "enabled": t.enabled }))
                .collect::<Vec<Value>>(),
        ),
    );
    Value::Object(out)
}

/// Una petición del cliente (con su capability asociada).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Listar los servidores.
    ListServers,
    /// Encender o apagar un servidor.
    ToggleServer {
        /// El servidor.
        name: String,
        /// El estado pedido.
        enabled: bool,
    },
    /// Encender o apagar una tool de un servidor.
    ToggleTool {
        /// El servidor.
        server: String,
        /// La tool.
        tool: String,
        /// El estado pedido.
        enabled: bool,
    },
}

impl Request {
    /// La capacidad que esta petición necesita (el opt-in se comprueba con
    /// ella).
    pub fn capability(&self) -> McpCapability {
        match self {
            Request::ListServers => McpCapability::ListServers,
            Request::ToggleServer { .. } => McpCapability::ToggleServer,
            Request::ToggleTool { .. } => McpCapability::ToggleTool,
        }
    }
}

/// El registro REAL de MCP (lo implementa el root).
///
/// El módulo sólo decide; el acceso (leer el `mcp_config.json`, escribirlo)
/// es del root.  Así los tests no dependen del runtime ni del disco.
pub trait McpRegistry {
    /// Lee el estado actual.
    fn read(&self) -> Result<McpState, McpControlError>;
    /// Persiste el estado nuevo (efecto secundario).
    fn write(&mut self, state: &McpState) -> Result<(), McpControlError>;
}

/// Un registro en memoria (tests y respaldo).
#[derive(Debug, Clone, Default)]
pub struct InMemoryRegistry {
    state: McpState,
    /// Cuántas veces se ha escrito (para comprobar que una operación
    /// idempotente NO toca el registro real).
    pub writes: usize,
}

impl InMemoryRegistry {
    /// Registro con un estado dado.
    pub fn new(state: McpState) -> Self {
        Self { state, writes: 0 }
    }

    /// El estado actual.
    pub fn state(&self) -> &McpState {
        &self.state
    }
}

impl McpRegistry for InMemoryRegistry {
    fn read(&self) -> Result<McpState, McpControlError> {
        Ok(self.state.clone())
    }

    fn write(&mut self, state: &McpState) -> Result<(), McpControlError> {
        self.state = state.clone();
        self.writes += 1;
        Ok(())
    }
}

/// El listado (copia pura del estado, en orden estable por nombre).
pub fn list_servers(state: &McpState) -> Vec<McpServer> {
    state.servers.values().cloned().collect()
}

// =============================================================================
// EL REGISTRO REAL: el `mcp_config.json` del móvil
// =============================================================================

/// La ruta del `mcp_config.json` donde se declaran los servidores MCP.
///
/// MEDIDO (2026-11-28): es la misma ruta que usa
/// `antigravity_clone::AGY_MCP_CONFIG`; se repite aquí para que este módulo
/// no dependa del runtime de Antigravity (y para que el **root** sea el que
/// implementa el registro real, como dice el trait).
pub const AGY_MCP_CONFIG: &str = "/data/user/0/com.agy/files/.gemini/config/mcp_config.json";

/// El estado REAL a partir del `mcp_config.json` (**puro**: sólo lee texto).
///
/// MEDIDO (2026-11-28): el formato es el de Gemini/Antigravity: un mapa
/// `mcpServers` donde cada servidor declara `url` (remoto) **o** `command` +
/// `args` (local).  El fichero **no** declara las tools (se descubren en
/// runtime, con `tools/list`) ⇒ la lista de tools de cada servidor sale
/// vacía y el consumidor lo dice (no se inventa).
///
/// El estado de cada servidor sale de `enabled` (nuestro) o de `disabled` (el
/// que usa Gemini): un servidor está apagado si alguno de los dos lo dice.
pub fn estado_desde_mcp_config(texto: &str) -> Result<McpState, McpControlError> {
    let v: Value = serde_json::from_str(texto).map_err(|e| McpControlError::MalformedState {
        reason: format!("el mcp_config no es JSON: {}", e),
    })?;
    let servidores = v
        .get("mcpServers")
        .and_then(|m| m.as_object())
        .ok_or_else(|| McpControlError::MalformedState {
            reason: "el mcp_config no tiene 'mcpServers'".into(),
        })?;
    let mut estado = McpState::new();
    for (nombre, s) in servidores {
        let obj = s
            .as_object()
            .ok_or_else(|| McpControlError::MalformedState {
                reason: format!("el servidor '{}' no es un objeto", nombre),
            })?;
        let target = if let Some(url) = obj.get("url").and_then(|u| u.as_str()) {
            ServerTarget::Url {
                url: url.to_string(),
            }
        } else if let Some(program) = obj.get("command").and_then(|c| c.as_str()) {
            let args = obj
                .get("args")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            ServerTarget::Command {
                program: program.to_string(),
                args,
            }
        } else {
            // Una entrada que no declara ni `url` ni `command` no es un
            // servidor: se salta (no se inventa un destino).
            continue;
        };
        let enabled = obj.get("enabled").and_then(|e| e.as_bool()).unwrap_or(true)
            && !obj
                .get("disabled")
                .and_then(|d| d.as_bool())
                .unwrap_or(false);
        estado.insert(McpServer {
            name: nombre.clone(),
            target,
            enabled,
            tools: Vec::new(),
        });
    }
    Ok(estado)
}

/// Aplica el estado al `mcp_config.json` (**puro**: devuelve el texto nuevo).
///
/// MEDIDO (2026-11-28): el fichero tiene **más** cosas (cabeceras, `env`,
/// `timeout`…), así que se parte del JSON **original** y sólo se toca el
/// estado del servidor (`enabled` / `disabled`).  Nada se borra.
pub fn aplicar_estado_a_mcp_config(
    texto: &str,
    estado: &McpState,
) -> Result<String, McpControlError> {
    let mut v: Value =
        serde_json::from_str(texto).map_err(|e| McpControlError::MalformedState {
            reason: format!("el mcp_config no es JSON: {}", e),
        })?;
    {
        let servidores = v
            .get_mut("mcpServers")
            .and_then(|m| m.as_object_mut())
            .ok_or_else(|| McpControlError::MalformedState {
                reason: "el mcp_config no tiene 'mcpServers'".into(),
            })?;
        for (nombre, s) in estado.servers.iter() {
            let Some(entrada) = servidores.get_mut(nombre).and_then(|e| e.as_object_mut()) else {
                continue;
            };
            entrada.insert("enabled".to_string(), json!(s.enabled));
            if s.enabled {
                // Encendido: fuera el `disabled` (el de Gemini).
                entrada.remove("disabled");
            } else {
                entrada.insert("disabled".to_string(), json!(true));
            }
        }
    }
    let mut salida = serde_json::to_string_pretty(&v).map_err(|e| McpControlError::Registry {
        reason: format!("serializar el mcp_config: {}", e),
    })?;
    salida.push('\n');
    Ok(salida)
}

/// El registro REAL: el `mcp_config.json` del disco.
///
/// Es la implementación del trait [`McpRegistry`] que el módulo dejaba
/// pendiente («lo implementa el root»).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRegistry {
    /// La ruta del fichero.
    pub ruta: PathBuf,
}

impl FileRegistry {
    /// El registro real por defecto (la config de Antigravity del móvil).
    pub fn real() -> Self {
        FileRegistry {
            ruta: PathBuf::from(AGY_MCP_CONFIG),
        }
    }

    /// El registro real de otra ruta (tests).
    pub fn en(ruta: impl Into<PathBuf>) -> Self {
        FileRegistry { ruta: ruta.into() }
    }
}

impl McpRegistry for FileRegistry {
    fn read(&self) -> Result<McpState, McpControlError> {
        let texto = std::fs::read_to_string(&self.ruta).map_err(|e| McpControlError::Registry {
            reason: format!("leer {}: {}", self.ruta.display(), e),
        })?;
        estado_desde_mcp_config(&texto)
    }

    fn write(&mut self, state: &McpState) -> Result<(), McpControlError> {
        let texto = std::fs::read_to_string(&self.ruta).map_err(|e| McpControlError::Registry {
            reason: format!("leer {}: {}", self.ruta.display(), e),
        })?;
        let salida = aplicar_estado_a_mcp_config(&texto, state)?;
        // MEDIDO (2026-11-28): se escribe **en el sitio** (truncar) y no con
        // rename: el fichero pertenece a la app (`u0_a1573`) y con `rename`
        // cambiaría el propietario (los permisos del móvil importan).  Es lo
        // mismo que hace `antigravity_clone::sincronizar_mcp_config`.
        std::fs::write(&self.ruta, salida.as_bytes()).map_err(|e| McpControlError::Registry {
            reason: format!("escribir {}: {}", self.ruta.display(), e),
        })
    }
}

/// Decide si hay que encender o apagar un servidor (puro: no muta nada).
///
/// * servidor desconocido ⇒ [`McpControlError::UnknownServer`];
/// * el servidor **ya está** en ese estado ⇒ decisión con `changed: false`
///   (idempotencia: **no** es un error).
pub fn plan_server_toggle(
    state: &McpState,
    name: &str,
    enabled: bool,
) -> Result<Decision, McpControlError> {
    let server = state
        .server(name)
        .ok_or_else(|| McpControlError::UnknownServer {
            name: name.to_string(),
        })?;
    let changed = server.enabled != enabled;
    Ok(Decision::Server {
        name: server.name.clone(),
        enabled,
        changed,
    })
}

/// Decide si hay que encender o apagar una tool de un servidor (puro).
///
/// El orden de las comprobaciones es deliberado:
///
/// 1. servidor desconocido ⇒ [`McpControlError::UnknownServer`];
/// 2. tool desconocida ⇒ [`McpControlError::UnknownTool`] (la petición del
///    cliente es incorrecta ⇒ se dice eso, no otra cosa);
/// 3. servidor apagado ⇒ [`McpControlError::ServerDisabled`] (cambiar una tool
///    de un servidor apagado **no** es un cambio efectivo: la whitelist se
///    escribiría sin que el servidor la use);
/// 4. la tool ya está en ese estado ⇒ `changed: false` (idempotencia).
pub fn plan_tool_toggle(
    state: &McpState,
    server: &str,
    tool: &str,
    enabled: bool,
) -> Result<Decision, McpControlError> {
    let srv = state
        .server(server)
        .ok_or_else(|| McpControlError::UnknownServer {
            name: server.to_string(),
        })?;
    let tool_entry = srv.tool(tool).ok_or_else(|| McpControlError::UnknownTool {
        server: server.to_string(),
        tool: tool.to_string(),
    })?;
    if !srv.enabled {
        return Err(McpControlError::ServerDisabled {
            server: server.to_string(),
        });
    }
    let changed = tool_entry.enabled != enabled;
    Ok(Decision::Tool {
        server: srv.name.clone(),
        tool: tool_entry.name.clone(),
        enabled,
        changed,
    })
}

/// Aplica la decisión al estado (muta el estado in-place).
///
/// Si la decisión no cambia nada, no se toca nada.
pub fn apply(state: &mut McpState, decision: &Decision) -> Result<(), McpControlError> {
    match decision {
        Decision::Listing(_) => Ok(()),
        Decision::Server {
            name,
            enabled,
            changed,
        } => {
            if !*changed {
                return Ok(());
            }
            let server = state
                .servers
                .get_mut(name)
                .ok_or_else(|| McpControlError::UnknownServer { name: name.clone() })?;
            server.enabled = *enabled;
            Ok(())
        }
        Decision::Tool {
            server,
            tool,
            enabled,
            changed,
        } => {
            if !*changed {
                return Ok(());
            }
            let srv =
                state
                    .servers
                    .get_mut(server)
                    .ok_or_else(|| McpControlError::UnknownServer {
                        name: server.clone(),
                    })?;
            let entry = srv
                .tools
                .iter_mut()
                .find(|t| t.name == *tool)
                .ok_or_else(|| McpControlError::UnknownTool {
                    server: server.clone(),
                    tool: tool.clone(),
                })?;
            entry.enabled = *enabled;
            Ok(())
        }
    }
}

/// Ejecuta una petición contra el registro real, respetando el opt-in mutuo.
///
/// 1. Si el cliente **no declaró** la capacidad de la petición ⇒ error
///    [`McpControlError::CapabilityNotDeclared`] y el registro no se toca.
/// 2. Se lee el estado, se **decide** (puro) y, si la decisión muta, se
///    escribe el estado nuevo (efecto secundario: una escritura al registro
///    real).  Una operación idempotente **no** escribe.
pub fn execute(
    registry: &mut dyn McpRegistry,
    declaration: &Declaration,
    request: &Request,
) -> Result<Decision, McpControlError> {
    let cap = request.capability();
    if !declaration.contains(cap) {
        return Err(McpControlError::CapabilityNotDeclared {
            capability: cap.id(),
        });
    }
    let state = registry.read()?;
    let decision = match request {
        Request::ListServers => Decision::Listing(list_servers(&state)),
        Request::ToggleServer { name, enabled } => plan_server_toggle(&state, name, *enabled)?,
        Request::ToggleTool {
            server,
            tool,
            enabled,
        } => plan_tool_toggle(&state, server, tool, *enabled)?,
    };
    if decision.changed() {
        let mut nuevo = state.clone();
        apply(&mut nuevo, &decision)?;
        registry.write(&nuevo)?;
    }
    Ok(decision)
}

/// El estado en el formato del `mcp_config.json` real (clave `mcpServers`).
///
/// La whitelist de tools (`enabled_tools`) es la que ya escribe
/// `service_bus::antigravity_bridge_config` (`rust-core/src/service_bus.rs:1564`).
pub fn state_to_json(state: &McpState) -> Value {
    let mut servers = serde_json::Map::new();
    for (name, server) in &state.servers {
        let mut entry = serde_json::Map::new();
        match &server.target {
            ServerTarget::Url { url } => {
                entry.insert("url".to_string(), json!(url));
            }
            ServerTarget::Command { program, args } => {
                entry.insert("command".to_string(), json!(program));
                if !args.is_empty() {
                    entry.insert("args".to_string(), json!(args));
                }
            }
        }
        entry.insert("enabled".to_string(), json!(server.enabled));
        let enabled_tools: Vec<&str> = server
            .tools
            .iter()
            .filter(|t| t.enabled)
            .map(|t| t.name.as_str())
            .collect();
        entry.insert("enabled_tools".to_string(), json!(enabled_tools));
        servers.insert(name.clone(), Value::Object(entry));
    }
    json!({ "mcpServers": Value::Object(servers) })
}

/// Lee el estado del formato `mcpServers`.
///
/// Acepta dos formas de lista de tools:
///
/// * `enabled_tools`: la whitelist (sólo dice cuáles están **habilitadas**;
///   el resto de tools del servidor las aporta el `tools/list` real, que es
///   lo que implementa el registro del root);
/// * `tools`: la lista completa con su estado (forma completa, la que usa
///   [`state_to_json`] al perder la whitelist).
pub fn state_from_json(value: &Value) -> Result<McpState, McpControlError> {
    let servers_obj = value
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .ok_or_else(|| McpControlError::MalformedState {
            reason: "falta el objeto 'mcpServers'".to_string(),
        })?;
    let mut state = McpState::new();
    for (name, entry) in servers_obj {
        let obj = entry
            .as_object()
            .ok_or_else(|| McpControlError::MalformedState {
                reason: format!("el servidor {name} no es un objeto"),
            })?;
        let target = if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
            ServerTarget::Url {
                url: url.to_string(),
            }
        } else if let Some(program) = obj.get("command").and_then(|v| v.as_str()) {
            ServerTarget::Command {
                program: program.to_string(),
                args: obj
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        } else {
            return Err(McpControlError::MalformedState {
                reason: format!("el servidor {name} no trae 'url' ni 'command'"),
            });
        };
        let enabled = obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
        let mut tools: Vec<McpTool> = Vec::new();
        if let Some(list) = obj.get("tools").and_then(|v| v.as_array()) {
            for t in list {
                let tname = t.get("name").and_then(|v| v.as_str()).unwrap_or("").trim();
                if tname.is_empty() {
                    return Err(McpControlError::MalformedState {
                        reason: format!("el servidor {name} tiene una tool sin nombre"),
                    });
                }
                let tenabled = t.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                tools.push(McpTool {
                    name: tname.to_string(),
                    enabled: tenabled,
                });
            }
        } else if let Some(list) = obj.get("enabled_tools").and_then(|v| v.as_array()) {
            for t in list {
                let tname = t.as_str().unwrap_or("").trim();
                if tname.is_empty() {
                    continue;
                }
                tools.push(McpTool {
                    name: tname.to_string(),
                    enabled: true,
                });
            }
        }
        state.insert(McpServer {
            name: name.clone(),
            target,
            enabled,
            tools,
        });
    }
    Ok(state)
}

/// El catálogo de la tool MCP de control (la añade `mcp_ext`).
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "mcp_control",
            "description": "Control de los servidores MCP y de sus tools: listar los servidores conocidos con el estado de cada tool, encender o apagar un servidor, y encender o apagar una tool concreta. Es idempotente: apagar un servidor ya apagado no es error; cambiar una tool de un servidor apagado sí lo es.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "accion": {
                        "type": "string",
                        "enum": ["listar", "toggle_servidor", "toggle_tool"],
                        "default": "listar",
                        "description": "Qué operación se quiere hacer"
                    },
                    "servidor": { "type": "string", "description": "Nombre del servidor MCP" },
                    "tool": { "type": "string", "description": "Nombre de la tool dentro del servidor" },
                    "habilitado": { "type": "boolean", "description": "Estado deseado (true = encender)" }
                },
                "required": ["accion"]
            }
        }
    ])
}

/// Las extensiones propias que el root registra con el namespace `mdaemon`.
pub fn extensiones() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "mcp/servers/list",
            "listar los servidores MCP y el estado (encendido/apagado) de sus tools",
        ),
        ("mcp/servers/toggle", "encender o apagar un servidor MCP"),
        (
            "mcp/tools/toggle",
            "encender o apagar una tool concreta de un servidor MCP",
        ),
    ]
}

/// El enganche con el dispatch REAL de `mcp_ext` (`rust-core/src/mcp_ext.rs:1180`).
///
/// Devuelve `None` si el nombre no es el de esta tool (para que el dispatch
/// siga su camino) y el sobre MCP (`content` + `isError`) si lo es.
pub fn dispatch_mcp(
    name: &str,
    args: &Value,
    registry: &mut dyn McpRegistry,
    declaration: &Declaration,
) -> Option<Value> {
    if name != "mcp_control" {
        return None;
    }
    let s = |k: &str| -> String {
        args.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let accion = {
        let a = s("accion");
        if a.is_empty() {
            "listar".to_string()
        } else {
            a
        }
    };
    let request = match accion.as_str() {
        "listar" => Request::ListServers,
        "toggle_servidor" => {
            let servidor = s("servidor");
            if servidor.is_empty() {
                return Some(error_envelope("falta 'servidor'"));
            }
            let habilitado = args
                .get("habilitado")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            Request::ToggleServer {
                name: servidor,
                enabled: habilitado,
            }
        }
        "toggle_tool" => {
            let servidor = s("servidor");
            let tool = s("tool");
            if servidor.is_empty() || tool.is_empty() {
                return Some(error_envelope("falta 'servidor' o 'tool'"));
            }
            let habilitado = args
                .get("habilitado")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            Request::ToggleTool {
                server: servidor,
                tool,
                enabled: habilitado,
            }
        }
        otro => return Some(error_envelope(&format!("acción desconocida: {otro}"))),
    };
    Some(match execute(registry, declaration, &request) {
        Ok(decision) => ok_envelope(&decision),
        Err(e) => error_envelope(&e.to_string()),
    })
}

fn ok_envelope(decision: &Decision) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&decision.to_json()).unwrap_or_default(),
        }],
    })
}

fn error_envelope(msg: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": msg }],
        "isError": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn servidor(nombre: &str, habilitado: bool, tools: &[(&str, bool)]) -> McpServer {
        McpServer {
            name: nombre.to_string(),
            target: ServerTarget::Url {
                url: format!("http://127.0.0.1:9090/{}", nombre),
            },
            enabled: habilitado,
            tools: tools
                .iter()
                .map(|(n, e)| McpTool {
                    name: n.to_string(),
                    enabled: *e,
                })
                .collect(),
        }
    }

    fn estado() -> McpState {
        let mut st = McpState::new();
        st.insert(servidor(
            "alpha",
            true,
            &[("buscar", true), ("escribir", true)],
        ));
        st.insert(servidor("beta", true, &[("leer", true)]));
        st.insert(servidor("gamma", false, &[("vieja", true)]));
        st
    }

    #[test]
    fn el_listado_devuelve_los_servidores_con_el_estado_de_sus_tools() {
        let listado = list_servers(&estado());
        assert_eq!(listado.len(), 3);
        assert_eq!(listado[0].name, "alpha");
        assert_eq!(listado[1].name, "beta");
        assert_eq!(listado[2].name, "gamma");
        assert!(listado[0].enabled);
        assert!(!listado[2].enabled);
        assert_eq!(listado[0].tools.len(), 2);
        assert!(listado[0].tools.iter().all(|t| t.enabled));
    }

    #[test]
    fn apagar_un_servidor_cambia_su_estado_y_se_persiste() {
        let mut reg = InMemoryRegistry::new(estado());
        let decl = Declaration::of(&[McpCapability::ToggleServer]);
        let d = execute(
            &mut reg,
            &decl,
            &Request::ToggleServer {
                name: "alpha".to_string(),
                enabled: false,
            },
        )
        .expect("debería apagar");
        assert_eq!(
            d,
            Decision::Server {
                name: "alpha".to_string(),
                enabled: false,
                changed: true
            }
        );
        assert!(!reg.state().server("alpha").unwrap().enabled);
        assert_eq!(reg.writes, 1, "un cambio real escribe en el registro");
    }

    #[test]
    fn encender_una_tool_no_toca_las_demas() {
        let mut reg = InMemoryRegistry::new(estado());
        let decl = Declaration::of(&[McpCapability::ToggleTool]);
        let d = execute(
            &mut reg,
            &decl,
            &Request::ToggleTool {
                server: "alpha".to_string(),
                tool: "escribir".to_string(),
                enabled: false,
            },
        )
        .expect("debería apagar la tool");
        assert_eq!(
            d,
            Decision::Tool {
                server: "alpha".to_string(),
                tool: "escribir".to_string(),
                enabled: false,
                changed: true
            }
        );
        let srv = reg.state().server("alpha").unwrap();
        assert!(!srv.tool("escribir").unwrap().enabled, "la tool apagada");
        assert!(srv.tool("buscar").unwrap().enabled, "la otra tool intacta");
        let beta = reg.state().server("beta").unwrap();
        assert!(beta.tool("leer").unwrap().enabled, "otro servidor intacto");
    }

    #[test]
    fn apagar_un_servidor_ya_apagado_es_idempotente_y_no_escribe() {
        let mut reg = InMemoryRegistry::new(estado());
        let decl = Declaration::of(&[McpCapability::ToggleServer]);
        let d = execute(
            &mut reg,
            &decl,
            &Request::ToggleServer {
                name: "gamma".to_string(),
                enabled: false,
            },
        )
        .expect("apagar lo ya apagado no es error");
        assert_eq!(
            d,
            Decision::Server {
                name: "gamma".to_string(),
                enabled: false,
                changed: false
            }
        );
        assert_eq!(
            reg.writes, 0,
            "una operación idempotente no toca el registro"
        );
    }

    #[test]
    fn una_tool_ya_en_ese_estado_tambien_es_idempotente() {
        let mut reg = InMemoryRegistry::new(estado());
        let decl = Declaration::of(&[McpCapability::ToggleTool]);
        let d = execute(
            &mut reg,
            &decl,
            &Request::ToggleTool {
                server: "alpha".to_string(),
                tool: "buscar".to_string(),
                enabled: true,
            },
        )
        .expect("no es error");
        assert!(!d.changed());
        assert_eq!(reg.writes, 0);
    }

    #[test]
    fn los_errores_de_servidor_y_tool_desconocidos_son_tipados() {
        let st = estado();
        match plan_server_toggle(&st, "no-existe", true) {
            Err(McpControlError::UnknownServer { name }) => assert_eq!(name, "no-existe"),
            otro => panic!("se esperaba UnknownServer, llegó {otro:?}"),
        }
        match plan_tool_toggle(&st, "alpha", "no-existe", true) {
            Err(McpControlError::UnknownTool { server, tool }) => {
                assert_eq!(server, "alpha");
                assert_eq!(tool, "no-existe");
            }
            otro => panic!("se esperaba UnknownTool, llegó {otro:?}"),
        }
        // Una tool de un servidor que no existe es un servidor desconocido.
        match plan_tool_toggle(&st, "no-existe", "buscar", true) {
            Err(McpControlError::UnknownServer { .. }) => {}
            otro => panic!("se esperaba UnknownServer, llegó {otro:?}"),
        }
    }

    #[test]
    fn cambiar_una_tool_de_un_servidor_apagado_es_error_tipado() {
        let st = estado();
        match plan_tool_toggle(&st, "gamma", "vieja", false) {
            Err(McpControlError::ServerDisabled { server }) => assert_eq!(server, "gamma"),
            otro => panic!("se esperaba ServerDisabled, llegó {otro:?}"),
        }
        // Ni aunque la tool ya esté apagada: el servidor apagado manda.
        match plan_tool_toggle(&st, "gamma", "vieja", true) {
            Err(McpControlError::ServerDisabled { .. }) => {}
            otro => panic!("se esperaba ServerDisabled, llegó {otro:?}"),
        }
    }

    #[test]
    fn sin_declaracion_no_se_anuncia_nada() {
        let decl = Declaration::none();
        assert!(decl.announced().is_empty());
        let mut reg = InMemoryRegistry::new(estado());
        match execute(&mut reg, &decl, &Request::ListServers) {
            Err(McpControlError::CapabilityNotDeclared { capability }) => {
                assert_eq!(capability, "list-servers");
            }
            otro => panic!("se esperaba CapabilityNotDeclared, llegó {otro:?}"),
        }
        assert_eq!(reg.writes, 0, "sin opt-in no se toca el registro");
    }

    #[test]
    fn solo_se_acepta_lo_que_el_cliente_declaro() {
        let decl = Declaration::of(&[McpCapability::ListServers]);
        assert_eq!(decl.announced(), &[McpCapability::ListServers]);
        let mut reg = InMemoryRegistry::new(estado());
        // Lo declarado funciona.
        let d = execute(&mut reg, &decl, &Request::ListServers).expect("listar está declarado");
        match d {
            Decision::Listing(servers) => assert_eq!(servers.len(), 3),
            otro => panic!("se esperaba un listado, llegó {otro:?}"),
        }
        // Lo NO declarado se rechaza con su identificador.
        match execute(
            &mut reg,
            &decl,
            &Request::ToggleServer {
                name: "alpha".to_string(),
                enabled: false,
            },
        ) {
            Err(McpControlError::CapabilityNotDeclared { capability }) => {
                assert_eq!(capability, "toggle-server");
            }
            otro => panic!("se esperaba CapabilityNotDeclared, llegó {otro:?}"),
        }
        assert_eq!(reg.writes, 0);
    }

    #[test]
    fn la_declaracion_se_parsea_y_se_rechaza_si_esta_rota() {
        let decl = Declaration::from_json(&json!(["list-servers", "toggle-tool"]))
            .expect("declaración válida");
        assert!(decl.contains(McpCapability::ListServers));
        assert!(decl.contains(McpCapability::ToggleTool));
        assert!(!decl.contains(McpCapability::ToggleServer));

        let decl = Declaration::from_json(&json!({ "capabilities": ["toggle-server"] }))
            .expect("forma con objeto");
        assert_eq!(decl.announced(), &[McpCapability::ToggleServer]);

        match Declaration::from_json(&json!(["capacidades-inventadas"])) {
            Err(McpControlError::MalformedDeclaration { reason }) => {
                assert!(reason.contains("capacidades-inventadas"))
            }
            otro => panic!("se esperaba MalformedDeclaration, llegó {otro:?}"),
        }
        match Declaration::from_json(&json!({ "capabilities": "list-servers" })) {
            Err(McpControlError::MalformedDeclaration { .. }) => {}
            otro => panic!("se esperaba MalformedDeclaration, llegó {otro:?}"),
        }
    }

    #[test]
    fn el_estado_va_y_vuelve_del_formato_mcpservers() {
        let st = estado();
        let json = state_to_json(&st);
        assert_eq!(json["mcpServers"]["alpha"]["enabled"], json!(true));
        assert_eq!(
            json["mcpServers"]["alpha"]["enabled_tools"],
            json!(["buscar", "escribir"])
        );
        assert_eq!(json["mcpServers"]["gamma"]["enabled"], json!(false));
        let de_vuelta = state_from_json(&json).expect("estado legible");
        assert_eq!(de_vuelta.servers.len(), 3);
        assert_eq!(de_vuelta.server("alpha").unwrap().tools.len(), 2);
        assert!(
            de_vuelta
                .server("alpha")
                .unwrap()
                .tool("buscar")
                .unwrap()
                .enabled
        );
        assert!(!de_vuelta.server("gamma").unwrap().enabled);
    }

    #[test]
    fn el_estado_roto_se_rechaza_con_error_tipado() {
        match state_from_json(&json!({})) {
            Err(McpControlError::MalformedState { reason }) => {
                assert!(reason.contains("mcpServers"))
            }
            otro => panic!("se esperaba MalformedState, llegó {otro:?}"),
        }
        match state_from_json(&json!({ "mcpServers": { "x": { "nombre": "y" } } })) {
            Err(McpControlError::MalformedState { .. }) => {}
            otro => panic!("se esperaba MalformedState, llegó {otro:?}"),
        }
    }

    #[test]
    fn el_dispatch_mcp_solo_atiende_a_su_tool() {
        let mut reg = InMemoryRegistry::new(estado());
        let decl = Declaration::of(&[McpCapability::ListServers]);
        assert!(dispatch_mcp("otra_tool", &json!({}), &mut reg, &decl).is_none());

        let sobre = dispatch_mcp(
            "mcp_control",
            &json!({ "accion": "listar" }),
            &mut reg,
            &decl,
        )
        .expect("responde");
        assert!(sobre.get("isError").is_none());
        let texto = sobre["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(
            texto.contains("alpha"),
            "el listado va en el texto: {texto}"
        );
    }

    #[test]
    fn el_dispatch_mcp_devuelve_error_tipado_como_sobre() {
        let mut reg = InMemoryRegistry::new(estado());
        let decl = Declaration::of(&[McpCapability::ToggleServer]);
        let sobre = dispatch_mcp(
            "mcp_control",
            &json!({ "accion": "toggle_servidor", "servidor": "no-existe" }),
            &mut reg,
            &decl,
        )
        .expect("responde");
        assert_eq!(sobre["isError"], json!(true));
        let texto = sobre["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(
            texto.contains("no-existe"),
            "el error dice qué pasó: {texto}"
        );
    }

    #[test]
    fn las_extensiones_propias_no_se_repiten() {
        let ext = extensiones();
        assert_eq!(ext.len(), 3);
        let mut ids: Vec<&str> = ext.iter().map(|(m, _)| *m).collect();
        ids.sort();
        let antes = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), antes, "los métodos se repiten");
        for (metodo, _) in &ext {
            assert!(metodo.starts_with("mcp/"), "namespace propio: {metodo}");
        }
    }

    // -----------------------------------------------------------------------
    // El registro REAL (el `mcp_config.json` del móvil)
    // -----------------------------------------------------------------------

    /// Un `mcp_config.json` de ejemplo (MEDIDO: el formato de Gemini).
    const CONFIG_EJEMPLO: &str = r#"{
      "mcpServers": {
        "megadaemon-minimax": {
          "url": "http://127.0.0.1:9090/mcp",
          "headers": { "X-API-Key": "secreto" }
        },
        "otro": {
          "command": "/data/adb/megadaemon/servidor.sh",
          "args": ["--stdio"],
          "env": { "CLAVE": "secreto" }
        },
        "apagado": { "command": "/bin/true", "disabled": true }
      }
    }"#;

    #[test]
    fn el_mcp_config_real_se_lee_como_registro() {
        // MEDIDO (2026-11-28): el `mcp_config.json` real (el de Antigravity)
        // es el registro MCP de verdad: `url` para los remotos y
        // `command`+`args` para los locales.
        let estado = estado_desde_mcp_config(CONFIG_EJEMPLO).expect("lee el config");
        assert_eq!(estado.servers.len(), 3);
        let remoto = estado.server("megadaemon-minimax").expect("el remoto");
        assert_eq!(
            remoto.target,
            ServerTarget::Url {
                url: "http://127.0.0.1:9090/mcp".to_string()
            }
        );
        assert!(remoto.enabled);
        let local = estado.server("otro").expect("el local");
        assert_eq!(
            local.target,
            ServerTarget::Command {
                program: "/data/adb/megadaemon/servidor.sh".to_string(),
                args: vec!["--stdio".to_string()],
            }
        );
        // `disabled` (el de Gemini) significa apagado.
        let apagado = estado.server("apagado").expect("el apagado");
        assert!(!apagado.enabled);
        // Las tools NO se declaran en el config (se descubren en runtime):
        // la lista sale vacía y no se inventa ninguna.
        assert!(remoto.tools.is_empty());
    }

    #[test]
    fn un_config_mal_formado_da_error_tipado() {
        assert!(matches!(
            estado_desde_mcp_config("esto no es json"),
            Err(McpControlError::MalformedState { .. })
        ));
        assert!(matches!(
            estado_desde_mcp_config("{}"),
            Err(McpControlError::MalformedState { .. })
        ));
    }

    #[test]
    fn escribir_el_estado_no_pierde_lo_que_no_se_toca() {
        // MEDIDO (2026-11-28): el config tiene cabeceras, `env` y compañía:
        // al escribir sólo se cambia el estado del servidor, NADA más.
        let mut estado = estado_desde_mcp_config(CONFIG_EJEMPLO).expect("lee");
        let apagado = estado.server("apagado").expect("el apagado").clone();
        let mut apagado2 = apagado.clone();
        apagado2.enabled = true;
        estado.insert(apagado2);
        let texto = aplicar_estado_a_mcp_config(CONFIG_EJEMPLO, &estado).expect("aplica");
        let v: Value = serde_json::from_str(&texto).expect("json");
        // Las cabeceras siguen ahí.
        assert_eq!(
            v["mcpServers"]["megadaemon-minimax"]["headers"]["X-API-Key"],
            json!("secreto")
        );
        // El `env` del local sigue ahí.
        assert_eq!(v["mcpServers"]["otro"]["env"]["CLAVE"], json!("secreto"));
        // Y el servidor que se ha encendido ya no está apagado.
        assert_eq!(v["mcpServers"]["apagado"]["enabled"], json!(true));
        assert!(v["mcpServers"]["apagado"].get("disabled").is_none());
    }

    #[test]
    fn el_registro_real_lee_y_escribe_el_fichero() {
        // El registro REAL (el que implementa el trait) sobre un fichero
        // de verdad: lee, apaga un servidor y lo comprueba.
        let ruta = std::env::temp_dir().join(format!(
            "mcp-control-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&ruta);
        std::fs::write(&ruta, CONFIG_EJEMPLO).expect("escribe el config");
        let mut reg = FileRegistry::en(&ruta);
        let mut estado = reg.read().expect("lee");
        let mut s = estado.server("otro").expect("el local").clone();
        s.enabled = false;
        estado.insert(s);
        reg.write(&estado).expect("escribe");
        let texto = std::fs::read_to_string(&ruta).expect("relee");
        let v: Value = serde_json::from_str(&texto).expect("json");
        assert_eq!(v["mcpServers"]["otro"]["enabled"], json!(false));
        assert_eq!(v["mcpServers"]["otro"]["disabled"], json!(true));
        // Y el resto sigue intacto.
        assert_eq!(
            v["mcpServers"]["otro"]["command"],
            json!("/data/adb/megadaemon/servidor.sh")
        );
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn el_catalogo_de_la_tool_declara_sus_acciones() {
        let cat = tool_definitions();
        let acciones = cat[0]["inputSchema"]["properties"]["accion"]["enum"]
            .as_array()
            .expect("enum de acciones");
        assert_eq!(acciones.len(), 3);
        assert_eq!(cat[0]["name"], json!("mcp_control"));
    }
}
