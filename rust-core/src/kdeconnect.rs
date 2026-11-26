//! Núcleo del protocolo **KDE Connect** en Rust puro.
//!
//! # Qué resuelve
//!
//! Hoy el canal estable del enjambre depende de que el **PC de casa** esté
//! encendido: si no lo está, el móvil queda ciego (el canónico
//! `mdtmfnc.duckdns.org` lo publica ese PC).  Este módulo da la pieza que
//! falta: un **protocolo de descubrimiento + identidad + emparejado** que
//! funciona en LAN, sin internet y sin terceros, con cifrado y con una
//! frontera de seguridad explícita (el PIN).
//!
//! # Formato de paquete (compatible con KDE Connect)
//!
//! ```json
//! {"id": "<uuid>", "type": "kdeconnect.pair", "body": {}}
//! ```
//!
//! Es el mismo esquema que usa la app oficial (`NetworkPacket::serialize()`
//! en `kdeconnect-kde`: `id` + `type` + `body`), salvo en el `id`: aquí es
//! un UUID v4 en vez del timestamp en milisegundos (el `id` oficial es un
//! `QVariant`, así que ambos lo aceptan; el nuestro sirve para correlacionar
//! `md.request` ↔ `md.response`).
//!
//! # Descubrimiento
//!
//! * **UDP en difusión** al puerto **1716** (el oficial de KDE Connect) con
//!   un paquete `kdeconnect.identity`, y al puerto **1776** (el del enjambre
//!   MegaDaemon) con un paquete `kdeconnect.discovery` de cuerpo idéntico.
//!   Los dos cuerpos llevan `deviceId`, `deviceName`, `deviceType`,
//!   `protocolVersion`, `incomingCapabilities`, `outgoingCapabilities`,
//!   `tcpPort` y `appName`.
//! * El datagram es **JSON plano**, sin framing (como en el oficial).
//!
//! # Transporte TCP
//!
//! * Framing **oficial**: JSON compacto + `\n` (verificado contra
//!   `kdeconnect-kde/core/networkpacket.cpp:59` y
//!   `kdeconnect-android/.../BoundedLineReader.kt`).
//! * Framing **alternativo** (el pedido del enjambre): **2 bytes
//!   big-endian de longitud + payload**.
//! * Los dos conviven: el *framing* se negocia en el paquete de conexión en
//!   claro (campo extra `framing`) y, si no viene, se autodetecta por los
//!   primeros bytes (`{` ⇒ newline, otra cosa ⇒ longitud).  Con esto un
//!   nodo MegaDaemon habla con la app oficial y con otro nodo MegaDaemon.
//!
//! # Cifrado
//!
//! El canal TCP va **cifrado con TLS** (`rustls`, con el proveedor `ring`
//! que ya está en el árbol).  Arranca en claro como en el oficial (el
//! paquete de identidad va en claro, es lo que dice el protocolo), y a
//! continuación **ambos extremos suben a TLS**:
//!
//! * el que **inició** la conexión TCP actúa como **servidor** TLS,
//! * el que la **aceptó** actúa como **cliente** TLS,
//!
//! igual que en KDE Connect (`protocol.md`, sección "Device Connection").
//!
//! Cada nodo presenta un **certificado autofirmado propio** (CN =
//! `deviceId`, clave Ed25519) y la confianza se hace por **pinning**: al
//! emparejarse, cada lado guarda la huella SHA-256 del certificado del
//! otro y a partir de ahí exige que sea **exactamente** la misma.  Es el
//! modelo de SSH (`known_hosts`), no el de una CA.
//!
//! # Frontera de seguridad (lo importante)
//!
//! * Un nodo **no emparejado no puede hacer nada**: el único paquete que se
//!   acepta de un par desconocido es `kdeconnect.pair`; el resto se tira.
//! * El empareado **exige PIN de 8 dígitos y confirmación explícita del
//!   usuario**.  Un PIN correcto sin confirmación humana es
//!   `NeedsUserConfirmation`, no `Accept`.
//! * La comparación del PIN es de **tiempo constante**.
//! * El PIN **nunca** se guarda en claro: se guarda su digest con sal, y
//!   sólo mientras la petición de empareado está pendiente.
//!
//! # Lo que NO hace este módulo
//!
//! * No habla con la app oficial de KDE Connect (eso exige el handshake
//!   completo de KDE Connect: ver `docs/KDE_CONNECT_Y_SIN_PC.md`).
//! * No implementa los plugins de la app oficial (notificaciones, SMS,
//!   portapapeles...).  Aquí sólo hay un canal de control mínimo y el
//!   `md.request` que delega en el daemon local.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ═══════════════════════════════════════════════════════════════════════════
// Constantes del protocolo
// ═══════════════════════════════════════════════════════════════════════════

/// Versión del protocolo KDE Connect que implementamos (la actual).
pub const PROTOCOL_VERSION: u16 = 8;

/// Puerto UDP **oficial** de descubrimiento de KDE Connect.
pub const UDP_PORT_OFFICIAL: u16 = 1716;

/// Puerto UDP **del enjambre** (el que pidió el usuario).
pub const UDP_PORT_MESH: u16 = 1776;

/// Los dos puertos en los que se escucha y se anuncia.
pub const UDP_PORTS: [u16; 2] = [UDP_PORT_OFFICIAL, UDP_PORT_MESH];

/// Rango de puertos TCP que KDE Connect usa (el oficial 1716..=1764).
pub const TCP_PORT_MIN: u16 = 1716;
/// Final del rango de puertos TCP de KDE Connect.
pub const TCP_PORT_MAX: u16 = 1764;

/// Puerto TCP por defecto de este módulo.
pub const TCP_PORT_DEFAULT: u16 = 1716;

/// Tamaño máximo de un paquete (el oficial usa 10 MiB).
pub const MAX_PACKET_SIZE: usize = 10 * 1024 * 1024;

/// Tamaño máximo del paquete de identidad en claro (el oficial, 64 KiB).
pub const MAX_IDENTITY_PACKET_SIZE: usize = 64 * 1024;

/// Longitud del PIN de empareado.
pub const PIN_DIGITS: usize = 8;

/// Ventana de validez de una petición de empareado (30 min, como el oficial).
pub const PAIR_REQUEST_TTL_S: u64 = 1_800;

/// Vida de una petición de empareado **pendiente** en el almacén.
pub const PENDING_PAIR_TTL_S: u64 = 300;

/// Tamaño máximo del cuerpo de un `md.request`.
pub const MD_REQUEST_MAX_BODY: usize = 1024 * 1024;

/// Tamaño máximo de la respuesta de un `md.request` que vuelve por el cable.
pub const MD_RESPONSE_MAX_BODY: usize = 256 * 1024;

/// Nombre de la aplicación que se anuncia en el descubrimiento.
pub const APP_NAME: &str = "MegaDaemon KDE Connect";

/// Tipos de paquete usados por este módulo.
pub mod ptype {
    /// Identidad (oficial).  En UDP = descubrimiento; en TCP = conexión.
    pub const IDENTITY: &str = "kdeconnect.identity";
    /// Descubrimiento del enjambre (alias de `identity` con otro tipo).
    pub const DISCOVERY: &str = "kdeconnect.discovery";
    /// Empareado (oficial, extendido con `pin`).
    pub const PAIR: &str = "kdeconnect.pair";
    /// Ping (oficial).
    pub const PING: &str = "kdeconnect.ping";
    /// Pong (respuesta al ping).
    pub const PONG: &str = "kdeconnect.pong";
    /// Eco (el plugin `mock.echo` del oficial).
    pub const ECHO: &str = "kdeconnect.mock.echo";
    /// Petición propia: "reenvía esto a tu daemon local".
    pub const MD_REQUEST: &str = "md.request";
    /// Respuesta propia a [`MD_REQUEST`].
    pub const MD_RESPONSE: &str = "md.response";
}

// ═══════════════════════════════════════════════════════════════════════════
// Aleatoriedad
// ═══════════════════════════════════════════════════════════════════════════

/// Bytes aleatorios criptográficos (usa `ring`, que ya está en el árbol).
pub fn random_bytes(n: usize) -> Vec<u8> {
    use ring::rand::SecureRandom;
    let mut out = vec![0u8; n];
    let rng = ring::rand::SystemRandom::new();
    // `SystemRandom` no puede fallar: si fallara es que el SO está roto.
    rng.fill(&mut out).expect("SystemRandom no puede fallar");
    out
}

/// UUID v4 en formato canónico (con guiones).
pub fn new_uuid() -> String {
    let mut b = random_bytes(16);
    b[6] = (b[6] & 0x0f) | 0x40; // versión 4
    b[8] = (b[8] & 0x3f) | 0x80; // variante RFC 4122
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// `deviceId` de KDE Connect: UUID v4 **sin guiones** (32 caracteres).
///
/// El oficial exige el patrón `^[a-zA-Z0-9_-]{32,38}$` y genera el id
/// quitando los guiones de un UUID v4 (`protocol.md`).
pub fn new_device_id() -> String {
    let mut b = random_bytes(16);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// PIN de 8 dígitos generado sin sesgo (rechazo, no módulo).
pub fn generate_pin() -> String {
    let mut out = String::with_capacity(PIN_DIGITS);
    while out.len() < PIN_DIGITS {
        for b in random_bytes(16) {
            if b < 250 {
                // 250 = 10 * 25: rechaza 6 de cada 256 para no sesgar.
                out.push(char::from(b'0' + (b % 10)));
                if out.len() == PIN_DIGITS {
                    break;
                }
            }
        }
    }
    out
}

/// Comparación de **tiempo constante** (PIN, huellas).
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ═══════════════════════════════════════════════════════════════════════════
// Paquete
// ═══════════════════════════════════════════════════════════════════════════

/// Paquete del protocolo: `{"id": ..., "type": ..., "body": {...}}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    /// Identificador del paquete (UUID v4).  Se usa para correlacionar.
    pub id: String,
    /// Tipo del paquete (`kdeconnect.pair`, `md.request`, ...).
    pub ptype: String,
    /// Cuerpo del paquete.
    pub body: Value,
}

impl Packet {
    /// Paquete nuevo con `id` autogenerado.
    pub fn new(ptype: &str, body: Value) -> Packet {
        Packet {
            id: new_uuid(),
            ptype: ptype.to_string(),
            body,
        }
    }

    /// Paquete con `id` explícito.
    pub fn with_id(id: &str, ptype: &str, body: Value) -> Packet {
        Packet {
            id: id.to_string(),
            ptype: ptype.to_string(),
            body,
        }
    }

    /// Serializa a JSON (el `body` siempre es un objeto).
    pub fn to_json(&self) -> Value {
        let body = if self.body.is_object() {
            self.body.clone()
        } else {
            json!({ "value": self.body })
        };
        json!({ "id": self.id, "type": self.ptype, "body": body })
    }

    /// Serializa a bytes JSON compactos (sin framing).
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.to_json()).unwrap_or_default()
    }

    /// Parsea un paquete desde JSON.
    ///
    /// El `id` puede ser string (el nuestro) o número (el oficial lo manda
    /// como timestamp en ms): se normaliza a string.
    pub fn from_json(v: &Value) -> Option<Packet> {
        let ptype = v.get("type").and_then(|t| t.as_str())?.to_string();
        if ptype.is_empty() {
            return None;
        }
        let id = match v.get("id") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => new_uuid(),
        };
        let body = v
            .get("body")
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        let body = if body.is_object() {
            body
        } else {
            json!({ "value": body })
        };
        Some(Packet { id, ptype, body })
    }

    /// Parsea un paquete desde bytes JSON.
    pub fn from_slice(b: &[u8]) -> Option<Packet> {
        let v: Value = serde_json::from_slice(b).ok()?;
        Packet::from_json(&v)
    }

    /// Campo del cuerpo como `&str`.
    pub fn str_field(&self, key: &str) -> Option<&str> {
        self.body.get(key).and_then(|v| v.as_str())
    }

    /// Campo del cuerpo como `u64`.
    pub fn u64_field(&self, key: &str) -> Option<u64> {
        self.body.get(key).and_then(|v| v.as_u64())
    }

    /// Campo del cuerpo como `bool`.
    pub fn bool_field(&self, key: &str) -> Option<bool> {
        self.body.get(key).and_then(|v| v.as_bool())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Framing del transporte TCP
// ═══════════════════════════════════════════════════════════════════════════

/// Framing del canal TCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    /// **Oficial de KDE Connect**: JSON compacto + `\n`.
    Newline,
    /// **Del enjambre**: 2 bytes big-endian de longitud + payload.
    LengthPrefixed,
}

impl Framing {
    /// Nombre corto (para negociar en el paquete de conexión).
    pub fn name(&self) -> &'static str {
        match self {
            Framing::Newline => "newline",
            Framing::LengthPrefixed => "length",
        }
    }

    /// Traduce el nombre recibido en el paquete de conexión.
    pub fn from_name(s: &str) -> Option<Framing> {
        match s {
            "newline" => Some(Framing::Newline),
            "length" => Some(Framing::LengthPrefixed),
            _ => None,
        }
    }

    /// Empaqueta un payload con este framing.
    pub fn encode(&self, payload: &[u8]) -> Vec<u8> {
        match self {
            Framing::Newline => {
                let mut v = Vec::with_capacity(payload.len() + 1);
                v.extend_from_slice(payload);
                v.push(b'\n');
                v
            }
            Framing::LengthPrefixed => {
                let n = payload.len().min(u16::MAX as usize) as u16;
                let mut v = Vec::with_capacity(payload.len() + 2);
                v.extend_from_slice(&n.to_be_bytes());
                v.extend_from_slice(payload);
                v
            }
        }
    }

    /// Autodetecta el framing por los primeros bytes.
    ///
    /// Un paquete JSON empieza por `{`; un prefijo de longitud de 2 bytes
    /// empieza por un byte alto (0x00 para los payloads cortos).
    pub fn detect(bytes: &[u8]) -> Option<Framing> {
        match bytes.first() {
            Some(b'{') => Some(Framing::Newline),
            Some(_) => Some(Framing::LengthPrefixed),
            None => None,
        }
    }
}

/// Error de troceado.
#[derive(Debug, Clone, PartialEq)]
pub enum FrameError {
    /// El paquete declarado supera el máximo permitido.
    TooLarge(usize),
    /// Longitud inválida en el prefijo.
    InvalidLength(usize),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::TooLarge(n) => write!(f, "paquete demasiado grande: {n} bytes"),
            FrameError::InvalidLength(n) => write!(f, "longitud de frame inválida: {n}"),
        }
    }
}

/// Troceador de frames: acumula bytes sueltos y devuelve payloads completos.
///
/// Tolera que los bytes lleguen troceados como quiera (requisito del
/// protocolo real: un `read()` puede devolver medio paquete).
#[derive(Debug, Clone)]
pub struct FrameDecoder {
    framing: Option<Framing>,
    buf: Vec<u8>,
    max_packet: usize,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        FrameDecoder::new()
    }
}

impl FrameDecoder {
    /// Troceador con framing conocido.
    pub fn with_framing(framing: Framing) -> FrameDecoder {
        FrameDecoder {
            framing: Some(framing),
            buf: Vec::new(),
            max_packet: MAX_PACKET_SIZE,
        }
    }

    /// Troceador con un tope de tamaño propio (para tests y para tuneles).
    pub fn with_max_packet(max_packet: usize) -> FrameDecoder {
        FrameDecoder {
            framing: None,
            buf: Vec::new(),
            max_packet,
        }
    }

    /// Troceador que autodetecta el framing en el primer frame.
    pub fn new() -> FrameDecoder {
        FrameDecoder {
            framing: None,
            buf: Vec::new(),
            max_packet: MAX_PACKET_SIZE,
        }
    }

    /// Fija el framing (desactiva la autodetección).
    pub fn set_framing(&mut self, framing: Framing) {
        self.framing = Some(framing);
    }

    /// Framing en uso (si ya se ha detectado).
    pub fn framing(&self) -> Option<Framing> {
        self.framing
    }

    /// Bytes pendientes en el búfer interno.
    pub fn pending(&self) -> usize {
        self.buf.len()
    }

    /// Añade bytes y devuelve los payloads completos que se puedan leer ya.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, FrameError> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            if self.framing.is_none() {
                match Framing::detect(&self.buf) {
                    Some(f) => self.framing = Some(f),
                    None => break, // aún no hay bytes
                }
            }
            match self.framing.expect("framing fijado") {
                Framing::Newline => {
                    if let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
                        let mut payload: Vec<u8> = self.buf.drain(..=pos).collect();
                        payload.pop(); // el '\n'
                        if payload.len() > self.max_packet {
                            return Err(FrameError::TooLarge(payload.len()));
                        }
                        out.push(payload);
                    } else {
                        if self.buf.len() > self.max_packet {
                            return Err(FrameError::TooLarge(self.buf.len()));
                        }
                        break;
                    }
                }
                Framing::LengthPrefixed => {
                    if self.buf.len() < 2 {
                        break;
                    }
                    let n = u16::from_be_bytes([self.buf[0], self.buf[1]]) as usize;
                    if n > self.max_packet {
                        return Err(FrameError::TooLarge(n));
                    }
                    if n == 0 {
                        return Err(FrameError::InvalidLength(0));
                    }
                    if self.buf.len() < n + 2 {
                        break;
                    }
                    let frame: Vec<u8> = self.buf.drain(..n + 2).collect();
                    out.push(frame[2..].to_vec());
                }
            }
        }
        Ok(out)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Identidad del nodo
// ═══════════════════════════════════════════════════════════════════════════

/// Identidad de un nodo (la que viaja en el descubrimiento y en el handshake).
#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    /// Id único del nodo (32 hex, patrón del oficial).
    pub device_id: String,
    /// Nombre legible (1..=32 caracteres saneados).
    pub device_name: String,
    /// Tipo (`phone`, `desktop`, ...).
    pub device_type: String,
    /// Versión del protocolo.
    pub protocol_version: u16,
    /// Puerto TCP donde escucha este nodo.
    pub tcp_port: u16,
    /// Tipos de paquete que este nodo **recibe**.
    pub incoming_capabilities: Vec<String>,
    /// Tipos de paquete que este nodo **envía**.
    pub outgoing_capabilities: Vec<String>,
    /// Nombre de la aplicación.
    pub app_name: String,
}

impl Default for Identity {
    fn default() -> Self {
        Identity {
            device_id: String::new(),
            device_name: String::new(),
            device_type: "desktop".to_string(),
            protocol_version: PROTOCOL_VERSION,
            tcp_port: TCP_PORT_DEFAULT,
            incoming_capabilities: default_capabilities(),
            outgoing_capabilities: default_capabilities(),
            app_name: APP_NAME.to_string(),
        }
    }
}

/// Capacidades que este módulo implementa (las dos mitades).
pub fn default_capabilities() -> Vec<String> {
    vec![
        ptype::PING.to_string(),
        ptype::PONG.to_string(),
        ptype::ECHO.to_string(),
        ptype::MD_REQUEST.to_string(),
        ptype::MD_RESPONSE.to_string(),
    ]
}

impl Identity {
    /// ¿Es válido un `deviceId` según el patrón del oficial?
    pub fn is_valid_device_id(id: &str) -> bool {
        let n = id.chars().count();
        (32..=38).contains(&n)
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    }

    /// Sanea el nombre del dispositivo como hace el oficial
    /// (`DeviceInfo::filterName`): quita `"',;:.!?()[]<>` y corta a 32.
    pub fn sanitize_device_name(name: &str) -> String {
        let filtered: String = name
            .chars()
            .filter(|c| {
                !matches!(
                    c,
                    '"' | '\'' | ';' | ':' | '.' | '!' | '?' | '(' | ')' | '[' | ']' | '<' | '>'
                )
            })
            .collect();
        filtered.chars().take(32).collect()
    }

    /// Cuerpo del paquete de descubrimiento / identidad.
    pub fn to_body(&self) -> Value {
        json!({
            "deviceId": self.device_id,
            "deviceName": self.device_name,
            "deviceType": self.device_type,
            "protocolVersion": self.protocol_version,
            "tcpPort": self.tcp_port,
            "incomingCapabilities": self.incoming_capabilities,
            "outgoingCapabilities": self.outgoing_capabilities,
            "appName": self.app_name,
        })
    }

    /// Paquete de descubrimiento UDP.
    ///
    /// `ptype` decide si es el oficial (`kdeconnect.identity`) o el del
    /// enjambre (`kdeconnect.discovery`).  El cuerpo es el mismo.
    pub fn to_discovery_packet(&self, ptype: &str) -> Packet {
        Packet {
            id: new_uuid(),
            ptype: ptype.to_string(),
            body: self.to_body(),
        }
    }

    /// Paquete de identidad completo (post-TLS).
    pub fn to_identity_packet(&self) -> Packet {
        self.to_discovery_packet(ptype::IDENTITY)
    }

    /// Paquete de conexión en claro (el primero del canal TCP).
    ///
    /// Incluye `targetDeviceId`/`targetProtocolVersion` (obligatorios en el
    /// oficial) y un campo extra `framing` para negociar el troceado.
    pub fn to_connection_packet(&self, target_device_id: &str, framing: Framing) -> Packet {
        let mut body = self.to_body();
        if let Some(obj) = body.as_object_mut() {
            obj.insert(
                "targetDeviceId".to_string(),
                Value::String(target_device_id.to_string()),
            );
            obj.insert(
                "targetProtocolVersion".to_string(),
                json!(self.protocol_version),
            );
            obj.insert(
                "framing".to_string(),
                Value::String(framing.name().to_string()),
            );
        }
        Packet {
            id: new_uuid(),
            ptype: ptype::IDENTITY.to_string(),
            body,
        }
    }

    /// Extrae la identidad de un paquete de descubrimiento o identidad.
    pub fn from_packet(p: &Packet) -> Option<Identity> {
        if p.ptype != ptype::IDENTITY && p.ptype != ptype::DISCOVERY {
            return None;
        }
        let device_id = p.str_field("deviceId")?.to_string();
        if !Identity::is_valid_device_id(&device_id) {
            return None;
        }
        let device_name = Identity::sanitize_device_name(p.str_field("deviceName").unwrap_or(""));
        let device_type = p.str_field("deviceType").unwrap_or("desktop").to_string();
        let protocol_version = p.u64_field("protocolVersion").unwrap_or(0) as u16;
        let tcp_port = p.u64_field("tcpPort").unwrap_or(0) as u16;
        let caps = |k: &str| -> Vec<String> {
            p.body
                .get(k)
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        };
        Some(Identity {
            device_id,
            device_name,
            device_type,
            protocol_version,
            tcp_port,
            incoming_capabilities: caps("incomingCapabilities"),
            outgoing_capabilities: caps("outgoingCapabilities"),
            app_name: p.str_field("appName").unwrap_or(APP_NAME).to_string(),
        })
    }
}

/// Identidad **persistente** de este nodo (vive en el `state_dir`).
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceIdentity {
    /// Id del nodo (no cambia nunca).
    pub device_id: String,
    /// Nombre del nodo (el usuario lo puede cambiar).
    pub device_name: String,
}

impl DeviceIdentity {
    /// Ruta del fichero de identidad.
    pub fn path(state_dir: &str) -> PathBuf {
        Path::new(state_dir).join("kde").join("device.json")
    }

    /// Carga la identidad; si no existe, la crea (id nuevo + nombre pedido).
    pub fn load_or_create(state_dir: &str, device_name: &str) -> std::io::Result<DeviceIdentity> {
        let path = DeviceIdentity::path(state_dir);
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                let id = v
                    .get("device_id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = v
                    .get("device_name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if Identity::is_valid_device_id(&id) && !name.is_empty() {
                    return Ok(DeviceIdentity {
                        device_id: id,
                        device_name: name,
                    });
                }
            }
        }
        let id = DeviceIdentity {
            device_id: new_device_id(),
            device_name: Identity::sanitize_device_name(device_name),
        };
        id.save(state_dir)?;
        Ok(id)
    }

    /// Guarda la identidad en disco (0600).
    pub fn save(&self, state_dir: &str) -> std::io::Result<()> {
        let path = DeviceIdentity::path(state_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
            set_dir_private(parent)?;
        }
        let body = json!({
            "device_id": self.device_id,
            "device_name": self.device_name,
        });
        std::fs::write(&path, body.to_string())?;
        set_file_private(&path)
    }

    /// Identidad completa (con tipo, puerto y capacidades).
    pub fn to_identity(&self, device_type: &str, tcp_port: u16) -> Identity {
        Identity {
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            device_type: device_type.to_string(),
            protocol_version: PROTOCOL_VERSION,
            tcp_port,
            incoming_capabilities: default_capabilities(),
            outgoing_capabilities: default_capabilities(),
            app_name: APP_NAME.to_string(),
        }
    }
}

/// Permisos 0600 para un fichero (o 0600 en Unix).
#[cfg(unix)]
fn set_file_private(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

/// Permisos 0600 para un fichero (en Windows no hay modo POSIX).
#[cfg(not(unix))]
fn set_file_private(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Permisos 0700 para un directorio.
#[cfg(unix)]
fn set_dir_private(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

/// Permisos 0700 para un directorio (en Windows no hay modo POSIX).
#[cfg(not(unix))]
fn set_dir_private(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Empareado (PIN)
// ═══════════════════════════════════════════════════════════════════════════

/// Petición de empareado (paquete `kdeconnect.pair`).
#[derive(Debug, Clone, PartialEq)]
pub struct PairRequest {
    /// `true` = petición/aceptación, `false` = rechazo o "desemparear".
    pub pair: bool,
    /// PIN de 8 dígitos (extensión propia: el oficial no lo lleva).
    pub pin: Option<String>,
    /// Segundos desde el epoch (el oficial lo usa para el código de
    /// verificación y para descartar peticiones viejas).
    pub timestamp: u64,
    /// Nombre del nodo que pide (para mostrarlo en la UI).
    pub device_name: Option<String>,
    /// Id del nodo que pide.
    pub device_id: Option<String>,
}

impl PairRequest {
    /// Lee una petición de empareado de un paquete.
    pub fn from_packet(p: &Packet) -> Option<PairRequest> {
        if p.ptype != ptype::PAIR {
            return None;
        }
        let pair = p.bool_field("pair")?;
        let pin = p
            .str_field("pin")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        Some(PairRequest {
            pair,
            pin,
            timestamp: p.u64_field("timestamp").unwrap_or(0),
            device_name: p.str_field("deviceName").map(|s| s.to_string()),
            device_id: p.str_field("deviceId").map(|s| s.to_string()),
        })
    }

    /// Escribe la petición (o la respuesta) como paquete.
    pub fn to_packet(&self) -> Packet {
        let mut body = Map::new();
        body.insert("pair".to_string(), Value::Bool(self.pair));
        body.insert("timestamp".to_string(), json!(self.timestamp));
        if let Some(pin) = &self.pin {
            body.insert("pin".to_string(), Value::String(pin.clone()));
        }
        if let Some(n) = &self.device_name {
            body.insert("deviceName".to_string(), Value::String(n.clone()));
        }
        if let Some(id) = &self.device_id {
            body.insert("deviceId".to_string(), Value::String(id.clone()));
        }
        Packet {
            id: new_uuid(),
            ptype: ptype::PAIR.to_string(),
            body: Value::Object(body),
        }
    }
}

/// Decisión sobre una petición de empareado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairDecision {
    /// Aceptada (y ya confirmada por el usuario).
    Accept,
    /// Rechazada explícitamente por el usuario.
    Deny,
    /// El PIN (si lo había) es correcto, pero **falta la confirmación
    /// humana**: todavía no se puede emparejar.
    NeedsUserConfirmation,
    /// El PIN no coincide.
    WrongPin,
    /// La petición no trae PIN y se exigía.
    MissingPin,
    /// La petición es demasiado vieja (más de 30 minutos).
    Expired,
}

impl PairDecision {
    /// ¿Se puede emparejar con esta decisión?
    pub fn is_accept(&self) -> bool {
        matches!(self, PairDecision::Accept)
    }
}

/// Política de empareado del nodo local.
#[derive(Debug, Clone)]
pub struct PairPolicy {
    /// ¿Se exige PIN?
    pub require_pin: bool,
    /// PIN esperado (si se conoce; si es `None` y `require_pin`, el PIN
    /// tiene que venir en la petición y basta con que el usuario confirme).
    pub expected_pin: Option<String>,
    /// ¿Se exige confirmación explícita del usuario?  **Siempre `true`**.
    pub require_user_confirmation: bool,
}

impl Default for PairPolicy {
    fn default() -> Self {
        PairPolicy {
            require_pin: true,
            expected_pin: None,
            require_user_confirmation: true,
        }
    }
}

/// Comprueba un PIN en **tiempo constante**.
pub fn verify_pin(expected: &str, received: &str) -> bool {
    constant_time_eq(expected.as_bytes(), received.as_bytes())
}

/// Decide qué hacer con una petición de empareado.
///
/// Reglas (en este orden):
///
/// 1. `pair == false` ⇒ `Deny` (desemparear / rechazo).
/// 2. Más de [`PAIR_REQUEST_TTL_S`] segundos ⇒ `Expired`.
/// 3. Si la política exige PIN:
///    * sin PIN en la petición ⇒ `MissingPin`;
///    * con PIN esperado y distinto ⇒ `WrongPin`;
/// 4. PIN correcto pero sin confirmación humana ⇒ `NeedsUserConfirmation`;
/// 5. si no ⇒ `Accept`.
pub fn decide_pairing(
    req: &PairRequest,
    policy: &PairPolicy,
    user_confirmed: bool,
    now: u64,
) -> PairDecision {
    if !req.pair {
        return PairDecision::Deny;
    }
    if req.timestamp > 0 && now.saturating_sub(req.timestamp) > PAIR_REQUEST_TTL_S {
        return PairDecision::Expired;
    }
    if policy.require_pin {
        let received = match req.pin.as_deref() {
            Some(p) if !p.is_empty() => p,
            _ => return PairDecision::MissingPin,
        };
        if let Some(expected) = policy.expected_pin.as_deref() {
            if !verify_pin(expected, received) {
                return PairDecision::WrongPin;
            }
        }
    }
    if policy.require_user_confirmation && !user_confirmed {
        return PairDecision::NeedsUserConfirmation;
    }
    PairDecision::Accept
}

/// Digest del PIN con sal (para **no** guardarlo en claro).
pub fn pin_digest(pin: &str, salt: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(salt);
    h.update(b"megadaemon-kde-pin-v1");
    h.update(pin.as_bytes());
    let out = h.finalize();
    let mut d = [0u8; 32];
    d.copy_from_slice(&out);
    d
}

// ═══════════════════════════════════════════════════════════════════════════
// Almacén de peers (confianza)
// ═══════════════════════════════════════════════════════════════════════════

/// Ficha de un peer emparejado.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PeerRecord {
    /// Id del peer.
    pub device_id: String,
    /// Nombre del peer.
    pub device_name: String,
    /// Huella SHA-256 (hex) de su certificado TLS (el "pin").
    pub cert_fingerprint: String,
    /// Epoch del empareado.
    pub paired_at: u64,
    /// Epoch de la última vez que se vio.
    pub last_seen: u64,
    /// ¿Sigue siendo de fiar?
    pub trusted: bool,
}

/// Almacén de peers emparejados (JSON en el `state_dir`, 0600).
#[derive(Debug, Clone)]
pub struct PeerStore {
    path: PathBuf,
    peers: BTreeMap<String, PeerRecord>,
}

impl PeerStore {
    /// Ruta del almacén.
    pub fn path(state_dir: &str) -> PathBuf {
        Path::new(state_dir).join("kde").join("peers.json")
    }

    /// Abre (sin leer) el almacén.
    pub fn open(state_dir: &str) -> PeerStore {
        PeerStore {
            path: PeerStore::path(state_dir),
            peers: BTreeMap::new(),
        }
    }

    /// Carga el almacén del disco (si no existe, queda vacío).
    pub fn load(&mut self) -> std::io::Result<()> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if let Some(obj) = v.get("peers").and_then(|p| p.as_object()) {
            for (id, rec) in obj {
                if let Ok(r) = serde_json::from_value::<PeerRecord>(rec.clone()) {
                    self.peers.insert(id.clone(), r);
                }
            }
        }
        Ok(())
    }

    /// Guarda el almacén (0600).
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
            set_dir_private(parent)?;
        }
        let peers: Map<String, Value> = self
            .peers
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::to_value(v).unwrap_or(Value::Null)))
            .collect();
        let body = json!({ "peers": Value::Object(peers) });
        std::fs::write(&self.path, body.to_string())?;
        set_file_private(&self.path)
    }

    /// Ficha de un peer.
    pub fn get(&self, device_id: &str) -> Option<&PeerRecord> {
        self.peers.get(device_id)
    }

    /// ¿Está el peer emparejado y de fiar?
    pub fn is_trusted(&self, device_id: &str) -> bool {
        self.peers
            .get(device_id)
            .map(|p| p.trusted)
            .unwrap_or(false)
    }

    /// Huella (pin) del certificado del peer, si está emparejado.
    pub fn fingerprint(&self, device_id: &str) -> Option<String> {
        self.peers
            .get(device_id)
            .filter(|p| p.trusted)
            .map(|p| p.cert_fingerprint.clone())
    }

    /// Empareja (o actualiza) un peer.
    pub fn trust(&mut self, rec: PeerRecord) -> std::io::Result<()> {
        self.peers.insert(rec.device_id.clone(), rec);
        self.save()
    }

    /// Revoca el empareado de un peer (inmediato, no hay que reiniciar).
    pub fn revoke(&mut self, device_id: &str) -> std::io::Result<bool> {
        let existed = self.peers.remove(device_id).is_some();
        if existed {
            self.save()?;
        }
        Ok(existed)
    }

    /// Marca que se ha visto a un peer (sin tocar la confianza).
    pub fn touch(&mut self, device_id: &str, now: u64) {
        if let Some(p) = self.peers.get_mut(device_id) {
            p.last_seen = now;
        }
    }

    /// Todos los peers (para listar).
    pub fn all(&self) -> Vec<&PeerRecord> {
        self.peers.values().collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Daemon local (destino del `md.request`)
// ═══════════════════════════════════════════════════════════════════════════

/// Configuración del daemon local al que se reenvían las peticiones `md.request`.
#[derive(Debug, Clone)]
pub struct LocalDaemon {
    /// Base URL local (por defecto `http://127.0.0.1:9090`).
    pub base_url: String,
    /// API key local (se lee del `state_dir`; **nunca** sale del nodo).
    pub api_key: Option<String>,
    /// Timeout de la petición local.
    pub timeout: Duration,
}

impl Default for LocalDaemon {
    fn default() -> Self {
        LocalDaemon {
            base_url: "http://127.0.0.1:9090".to_string(),
            api_key: None,
            timeout: Duration::from_secs(15),
        }
    }
}

impl LocalDaemon {
    /// Construye el descriptor leyendo la API key del `state_dir`.
    pub fn from_state_dir(state_dir: &str) -> LocalDaemon {
        let mut d = LocalDaemon::default();
        // La API key se queda en el nodo: sólo se usa para llamar al
        // daemon local (que además exime al loopback).
        if let Ok(text) =
            std::fs::read_to_string(Path::new(state_dir).join("megadaemon_api.key"))
        {
            let k = text.trim().to_string();
            if !k.is_empty() {
                d.api_key = Some(k);
            }
        }
        d
    }

    /// Reenvía una petición HTTP al daemon local y devuelve la respuesta.
    ///
    /// `path` debe ser una ruta **relativa** (empieza por `/`): el host es
    /// siempre el del daemon local, así que un peer no puede hacer SSRF.
    pub fn forward(&self, method: &str, path: &str, body: Option<&str>) -> Result<HttpReply, String> {
        if !path.starts_with('/') || path.contains("://") || path.contains('\n') {
            return Err("ruta inválida en md.request".into());
        }
        if path.len() > 2048 {
            return Err("ruta demasiado larga en md.request".into());
        }
        let method = method.to_ascii_uppercase();
        if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "DELETE" | "HEAD") {
            return Err(format!("método no permitido en md.request: {method}"));
        }
        if let Some(b) = body {
            if b.len() > MD_REQUEST_MAX_BODY {
                return Err("cuerpo demasiado grande en md.request".into());
            }
        }
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            .timeout_connect(Some(Duration::from_secs(3)))
            // Un 404 del daemon local también es información útil para el
            // peer: con `http_status_as_error` (por defecto) ureq tiraría
            // el cuerpo y sólo dejaría el código.
            .http_status_as_error(false)
            .build();
        let agent = ureq::Agent::new_with_config(config);

        // `ureq` usa dos tipos de builder (con cuerpo / sin cuerpo), así que
        // cada rama construye el suyo y llama a su propio envío.
        let result = match method.as_str() {
            "GET" => with_headers(agent.get(&url), self.api_key.as_deref()).call(),
            "DELETE" => with_headers(agent.delete(&url), self.api_key.as_deref()).call(),
            "HEAD" => with_headers(agent.head(&url), self.api_key.as_deref()).call(),
            "POST" | "PUT" => {
                let req = with_headers(agent.post(&url), self.api_key.as_deref());
                match body {
                    Some(b) => req.send(b.as_bytes()),
                    None => req.send_empty(),
                }
            }
            _ => return Err(format!("método no permitido en md.request: {method}")),
        };

        match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let content_type = resp
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                let (text, truncated) = read_capped(resp.into_body());
                Ok(HttpReply {
                    status,
                    body: text,
                    content_type,
                    truncated,
                })
            }
            Err(e) => Err(format!("md.request: el daemon local no respondió: {e}")),
        }
    }
}

/// Añade las cabeceras de autenticación y de canal a una petición.
///
/// Genérico a propósito: `ureq` tiene dos tipos de `RequestBuilder`
/// (con cuerpo y sin cuerpo) y los dos implementan `header`.
fn with_headers<B>(
    mut req: ureq::RequestBuilder<B>,
    api_key: Option<&str>,
) -> ureq::RequestBuilder<B> {
    if let Some(key) = api_key {
        req = req.header("X-API-Key", key);
    }
    req.header("X-MegaDaemon-Channel", "kdeconnect")
}

/// Lee el cuerpo de una respuesta HTTP local con tope de tamaño.
fn read_capped(mut body: ureq::Body) -> (String, bool) {
    let mut text = match body.read_to_string() {
        Ok(t) => t,
        Err(_) => String::new(),
    };
    let mut truncated = false;
    if text.len() > MD_RESPONSE_MAX_BODY {
        // Corte por carácter (no por byte) para no partir un UTF-8.
        let cut = text
            .char_indices()
            .nth(MD_RESPONSE_MAX_BODY)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        text.truncate(cut);
        truncated = true;
    }
    (text, truncated)
}

/// Respuesta de un `md.request`.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpReply {
    /// Código HTTP.
    pub status: u16,
    /// Cuerpo (recortado a [`MD_RESPONSE_MAX_BODY`]).
    pub body: String,
    /// `Content-Type` (si lo hubo).
    pub content_type: Option<String>,
    /// ¿Se recortó el cuerpo?
    pub truncated: bool,
}

// ═══════════════════════════════════════════════════════════════════════════
// Canal de control
// ═══════════════════════════════════════════════════════════════════════════

/// Contexto en el que se resuelve un paquete recibido.
pub struct ControlContext<'a> {
    /// Identidad del nodo local.
    pub identity: &'a Identity,
    /// Almacén de peers (para decidir si el peer está emparejado).
    pub peers: &'a PeerStore,
    /// Daemon local al que se pueden reenviar peticiones.
    pub local_daemon: Option<&'a LocalDaemon>,
    /// Id del peer que manda el paquete.
    pub peer_id: String,
    /// ¿El peer está emparejado?
    pub peer_trusted: bool,
}

/// Resuelve un paquete de control y devuelve la respuesta (si la hay).
///
/// * `kdeconnect.ping` → `kdeconnect.pong`
/// * `kdeconnect.mock.echo` → `kdeconnect.mock.echo`
/// * `md.request` → `md.response`
///
/// **Sólo si el peer está emparejado**: cualquier paquete de un peer sin
/// emparejar (y cualquier tipo desconocido) se **descarta**: `None`.
pub fn handle_control(packet: &Packet, ctx: &ControlContext<'_>) -> Option<Packet> {
    // FRONTERA DE SEGURIDAD: sin emparejar, el peer no obtiene NADA.
    // En KDE Connect oficial los paquetes de un dispositivo no emparejado
    // se descartan todos salvo los de pareado (`protocol.md`).
    if !ctx.peer_trusted {
        return None;
    }
    match packet.ptype.as_str() {
        ptype::PING => {
            let msg = packet.str_field("message").unwrap_or("").to_string();
            Some(Packet::new(
                ptype::PONG,
                json!({ "message": msg, "deviceName": ctx.identity.device_name }),
            ))
        }
        ptype::ECHO => {
            let msg = packet.str_field("message").unwrap_or("").to_string();
            Some(Packet::new(ptype::ECHO, json!({ "message": msg })))
        }
        ptype::MD_REQUEST => {
            let daemon = ctx.local_daemon?;
            let method = packet.str_field("method").unwrap_or("GET").to_string();
            let path = packet.str_field("path").unwrap_or("/").to_string();
            let body = packet.str_field("body").map(|s| s.to_string());
            // La respuesta **conserva el id de la petición**: así el que
            // preguntó sabe que la respuesta es la suya y no la de otro.
            match daemon.forward(&method, &path, body.as_deref()) {
                Ok(reply) => Some(Packet::with_id(
                    &packet.id,
                    ptype::MD_RESPONSE,
                    json!({
                        "status": reply.status,
                        "body": reply.body,
                        "contentType": reply.content_type,
                        "truncated": reply.truncated,
                    }),
                )),
                Err(e) => Some(Packet::with_id(
                    &packet.id,
                    ptype::MD_RESPONSE,
                    json!({ "status": 0, "body": e, "truncated": false }),
                )),
            }
        }
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TLS
// ═══════════════════════════════════════════════════════════════════════════

/// Material TLS del nodo (certificado + clave), persistido en el `state_dir`.
#[derive(Clone)]
pub struct TlsMaterial {
    /// Certificado DER.
    pub cert_der: Vec<u8>,
    /// Clave privada en PKCS#8.
    pub key_pkcs8: Vec<u8>,
    /// Huella SHA-256 del certificado (el "pin").
    pub fingerprint: String,
}

impl std::fmt::Debug for TlsMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsMaterial")
            .field("fingerprint", &self.fingerprint)
            .finish()
    }
}

impl TlsMaterial {
    /// Ruta del certificado.
    pub fn cert_path(state_dir: &str) -> PathBuf {
        Path::new(state_dir).join("kde").join("cert.pem")
    }

    /// Ruta de la clave privada.
    pub fn key_path(state_dir: &str) -> PathBuf {
        Path::new(state_dir).join("kde").join("key.pem")
    }

    /// Carga el material TLS; si no existe, lo genera (autofirmado).
    pub fn load_or_create(state_dir: &str, device_id: &str) -> std::io::Result<TlsMaterial> {
        let cert_path = TlsMaterial::cert_path(state_dir);
        let key_path = TlsMaterial::key_path(state_dir);
        if let (Ok(cert_pem), Ok(key_pem)) = (
            std::fs::read_to_string(&cert_path),
            std::fs::read_to_string(&key_path),
        ) {
            if let (Some(der), Some(key)) = (
                crate::selfsigned_cert::from_pem(&cert_pem),
                crate::selfsigned_cert::key_from_pem(&key_pem),
            ) {
                let fingerprint = crate::selfsigned_cert::fingerprint_hex(&der);
                return Ok(TlsMaterial {
                    cert_der: der,
                    key_pkcs8: key,
                    fingerprint,
                });
            }
        }
        let cert = crate::selfsigned_cert::generate(device_id, 3650)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        if let Some(parent) = cert_path.parent() {
            std::fs::create_dir_all(parent)?;
            set_dir_private(parent)?;
        }
        std::fs::write(&cert_path, crate::selfsigned_cert::to_pem(&cert.der))?;
        set_file_private(&cert_path)?;
        std::fs::write(&key_path, crate::selfsigned_cert::key_to_pem(&cert.key_pkcs8))?;
        set_file_private(&key_path)?;
        Ok(TlsMaterial {
            cert_der: cert.der,
            key_pkcs8: cert.key_pkcs8,
            fingerprint: cert.fingerprint,
        })
    }

    /// Configuración de servidor TLS (presentamos nuestro certificado).
    ///
    /// Se **pide** certificado al cliente (TLS mutuo, como en KDE Connect) y
    /// se acepta cualquiera (TOFU): la comprobación real del *pin* la hace
    /// la capa de sesión, que es donde se sabe qué par está emparejado.
    pub fn server_config(&self) -> Result<Arc<rustls::ServerConfig>, String> {
        use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let certs = vec![CertificateDer::from(self.cert_der.clone())];
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key_pkcs8.clone()));
        let cfg = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| format!("TLS: no se pudieron elegir versiones: {e}"))?
            .with_client_cert_verifier(Arc::new(AcceptAnyClientCert))
            .with_single_cert(certs, key)
            .map_err(|e| format!("TLS: el certificado propio no sirve: {e}"))?;
        Ok(Arc::new(cfg))
    }

    /// Configuración de cliente TLS con **pinning** del certificado del peer.
    ///
    /// * si hay pin guardado ⇒ sólo se acepta ese certificado exacto;
    /// * si no hay pin (peer sin emparejar) ⇒ se acepta cualquiera, pero el
    ///   peer **no puede hacer nada** hasta emparejarse (ver
    ///   [`handle_control`]): sólo se le acepta `kdeconnect.pair`.
    pub fn client_config(&self, pinned: Option<&str>) -> Result<Arc<rustls::ClientConfig>, String> {
        use rustls::client::danger::ServerCertVerifier;
        use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
        use rustls::{ClientConfig, Error as TlsError, SignatureScheme};

        #[derive(Debug)]
        struct PinnedVerifier {
            expected: Option<Vec<u8>>,
        }

        impl ServerCertVerifier for PinnedVerifier {
            fn verify_server_cert(
                &self,
                end_entity: &CertificateDer<'_>,
                _intermediates: &[CertificateDer<'_>],
                _server_name: &ServerName<'_>,
                _ocsp_response: &[u8],
                _now: UnixTime,
            ) -> Result<rustls::client::danger::ServerCertVerified, TlsError> {
                let fp = crate::selfsigned_cert::fingerprint_hex(end_entity.as_ref());
                match &self.expected {
                    Some(exp) => {
                        if constant_time_eq(exp.as_slice(), fp.as_bytes()) {
                            Ok(rustls::client::danger::ServerCertVerified::assertion())
                        } else {
                            Err(TlsError::General(
                                "el certificado del peer no es el que se emparejó".into(),
                            ))
                        }
                    }
                    // Sin pin: sólo se permite el handshake para poder
                    // emparejarse.  Ningún otro paquete se acepta sin
                    // emparejar (eso lo decide `handle_control`).
                    None => Ok(rustls::client::danger::ServerCertVerified::assertion()),
                }
            }

            fn verify_tls12_signature(
                &self,
                message: &[u8],
                cert: &CertificateDer<'_>,
                dss: &rustls::DigitallySignedStruct,
            ) -> Result<rustls::client::danger::HandshakeSignatureValid, TlsError> {
                rustls::crypto::verify_tls12_signature(
                    message,
                    cert,
                    dss,
                    &rustls::crypto::ring::default_provider().signature_verification_algorithms,
                )
            }

            fn verify_tls13_signature(
                &self,
                message: &[u8],
                cert: &CertificateDer<'_>,
                dss: &rustls::DigitallySignedStruct,
            ) -> Result<rustls::client::danger::HandshakeSignatureValid, TlsError> {
                rustls::crypto::verify_tls13_signature(
                    message,
                    cert,
                    dss,
                    &rustls::crypto::ring::default_provider().signature_verification_algorithms,
                )
            }

            fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
                rustls::crypto::ring::default_provider()
                    .signature_verification_algorithms
                    .supported_schemes()
            }
        }

        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = Arc::new(PinnedVerifier {
            expected: pinned.map(|p| p.as_bytes().to_vec()),
        });
        // TLS mutuo: también mandamos nuestro certificado, para que el par
        // (que en este rol es el servidor TLS) pueda pinnearlo.
        let certs = vec![rustls::pki_types::CertificateDer::from(self.cert_der.clone())];
        let key = rustls::pki_types::PrivateKeyDer::Pkcs8(
            rustls::pki_types::PrivatePkcs8KeyDer::from(self.key_pkcs8.clone()),
        );
        let cfg = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| format!("TLS: no se pudieron elegir versiones: {e}"))?
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_client_auth_cert(certs, key)
            .map_err(|e| format!("TLS: no se pudo mandar el certificado propio: {e}"))?;
        Ok(Arc::new(cfg))
    }
}

/// Verificador de certificado de cliente que acepta cualquiera (TOFU).
///
/// El TLS no puede decidir si el par es de fiar (no hay CA): eso lo decide
/// la **capa de sesión**, comparando la huella del certificado con el *pin*
/// guardado al emparejar.  Aquí sólo se comprueba que el certificado es
/// utilizable y que firma bien el handshake.
#[derive(Debug)]
struct AcceptAnyClientCert;

impl rustls::server::danger::ClientCertVerifier for AcceptAnyClientCert {
    fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
        &[]
    }

    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> bool {
        // No se obliga: un par sin certificado (p. ej. una implementación
        // que no lo mande) puede conectarse, pero entonces no se puede
        // emparejar (no hay huella que pinnear).
        false
    }

    fn verify_client_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
        Ok(rustls::server::danger::ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Flujo TLS (cliente o servidor) sobre un `TcpStream`.
enum Secure {
    /// Somos el cliente TLS (aceptamos la conexión TCP).
    Client(rustls::StreamOwned<rustls::ClientConnection, TcpStream>),
    /// Somos el servidor TLS (iniciamos la conexión TCP).
    Server(rustls::StreamOwned<rustls::ServerConnection, TcpStream>),
}

/// Termina el handshake TLS sobre el `TcpStream` (que todavía está fuera del
/// `StreamOwned`).
///
/// El socket tiene timeout de lectura: si el peer no contesta, el error es
/// de tiempo, no un cuelgue.
fn complete_tls_io(conn: &mut rustls::ClientConnection, stream: &mut TcpStream) -> Result<(), String> {
    tls_io_result(conn.complete_io(stream))
}

/// Igual, pero en el rol de servidor TLS.
fn complete_tls_io_server(
    conn: &mut rustls::ServerConnection,
    stream: &mut TcpStream,
) -> Result<(), String> {
    tls_io_result(conn.complete_io(stream))
}

/// Traduce el resultado del handshake a un mensaje utilizable.
fn tls_io_result(r: std::io::Result<(usize, usize)>) -> Result<(), String> {
    match r {
        Ok(_) => Ok(()),
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
        {
            Err("TLS: el handshake se quedó sin tiempo".into())
        }
        Err(e) => Err(format!("TLS: handshake fallido: {e}")),
    }
}

/// Lectura y escritura sobre el flujo TLS.
impl Secure {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Secure::Client(s) => s.read(buf),
            Secure::Server(s) => s.read(buf),
        }
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        match self {
            Secure::Client(s) => s.write_all(buf),
            Secure::Server(s) => s.write_all(buf),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Sesión
// ═══════════════════════════════════════════════════════════════════════════

/// Configuración de una sesión.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Framing preferido (si es `None`, se usa el que indique el peer).
    pub framing: Option<Framing>,
    /// Timeout de las operaciones de red.
    pub timeout: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        SessionConfig {
            framing: None,
            timeout: Duration::from_secs(10),
        }
    }
}

/// Sesión con un peer: identidad, confianza y canal de control.
pub struct Session {
    /// Identidad del peer.
    pub peer: Identity,
    /// ¿El peer está emparejado?
    pub trusted: bool,
    /// Framing en uso.
    pub framing: Framing,
    /// Identidad local.
    pub local: Identity,
    /// Huella SHA-256 del certificado que presentó el peer (si lo presentó).
    pub peer_cert_fingerprint: Option<String>,
    secure: Secure,
    decoder: FrameDecoder,
}

impl Session {
    /// Conecta con un peer (somos el **servidor** TLS, como en el oficial).
    pub fn connect(
        addr: SocketAddr,
        target_device_id: &str,
        local: &Identity,
        material: &TlsMaterial,
        peers: &PeerStore,
        cfg: &SessionConfig,
    ) -> Result<Session, String> {
        let mut stream = TcpStream::connect_timeout(&addr, cfg.timeout)
            .map_err(|e| format!("no se pudo conectar a {addr}: {e}"))?;
        stream
            .set_read_timeout(Some(cfg.timeout))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(cfg.timeout))
            .map_err(|e| e.to_string())?;

        let framing = cfg.framing.unwrap_or(Framing::Newline);
        let conn_pkt = local.to_connection_packet(target_device_id, framing);
        stream
            .write_all(&Framing::Newline.encode(&conn_pkt.to_bytes()))
            .map_err(|e| format!("no se pudo enviar la identidad: {e}"))?;
        stream.flush().ok();

        // La identidad del peer llega en claro (como en el oficial).
        let raw = read_line_bounded(&mut stream, MAX_IDENTITY_PACKET_SIZE)
            .map_err(|e| format!("no se pudo leer la identidad del peer: {e}"))?;
        let peer_conn = Packet::from_slice(&raw).ok_or("la identidad del peer no es un paquete")?;
        if peer_conn.ptype != ptype::IDENTITY {
            return Err("el peer no mandó un paquete de identidad".into());
        }
        let target = peer_conn.str_field("targetDeviceId").unwrap_or("");
        if !target.is_empty() && target != local.device_id {
            return Err(format!(
                "el peer se conectó pensando que era otro nodo (target {target})"
            ));
        }
        // Framing negociado: el que dice el peer (si no dice, el oficial).
        let framing = peer_conn
            .str_field("framing")
            .and_then(Framing::from_name)
            .unwrap_or(framing);

        // TLS: como iniciadores del TCP, somos el SERVIDOR TLS.
        let server_cfg = material.server_config()?;
        let mut conn = rustls::ServerConnection::new(server_cfg).map_err(|e| e.to_string())?;
        complete_tls_io_server(&mut conn, &mut stream)?;
        // El certificado del peer (nos lo manda como cliente TLS).
        let peer_cert_fingerprint = conn
            .peer_certificates()
            .and_then(|c| c.first())
            .map(|c| crate::selfsigned_cert::fingerprint_hex(c.as_ref()));
        let mut secure = Secure::Server(rustls::StreamOwned::new(conn, stream));

        let mut decoder = FrameDecoder::with_framing(framing);
        // Identidad completa, ya cifrada.
        secure
            .write_all(&framing.encode(&local.to_identity_packet().to_bytes()))
            .map_err(|e| format!("no se pudo enviar la identidad cifrada: {e}"))?;
        let peer_raw = read_packet(&mut secure, &mut decoder)?;
        let peer_pkt =
            Packet::from_slice(&peer_raw).ok_or("la identidad cifrada del peer no es un paquete")?;
        let peer = Identity::from_packet(&peer_pkt)
            .ok_or("la identidad cifrada del peer no es válida")?;
        let trusted = peers.is_trusted(&peer.device_id);
        check_pin(peers, &peer.device_id, peer_cert_fingerprint.as_deref())?;

        Ok(Session {
            peer,
            trusted,
            framing,
            local: local.clone(),
            peer_cert_fingerprint,
            secure,
            decoder,
        })
    }

    /// Acepta una conexión entrante (somos el **cliente** TLS).
    pub fn accept(
        mut stream: TcpStream,
        local: &Identity,
        material: &TlsMaterial,
        peers: &PeerStore,
        cfg: &SessionConfig,
    ) -> Result<Session, String> {
        stream
            .set_read_timeout(Some(cfg.timeout))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(cfg.timeout))
            .map_err(|e| e.to_string())?;

        let raw = read_line_bounded(&mut stream, MAX_IDENTITY_PACKET_SIZE)
            .map_err(|e| format!("no se pudo leer la identidad del peer: {e}"))?;
        let peer_conn = Packet::from_slice(&raw).ok_or("la identidad del peer no es un paquete")?;
        if peer_conn.ptype != ptype::IDENTITY {
            return Err("el peer no mandó un paquete de identidad".into());
        }
        let target = peer_conn.str_field("targetDeviceId").unwrap_or("");
        if !target.is_empty() && target != local.device_id {
            return Err(format!(
                "el peer se conectó pensando que era otro nodo (target {target})"
            ));
        }
        let framing = peer_conn
            .str_field("framing")
            .and_then(Framing::from_name)
            .unwrap_or(Framing::Newline);
        let conn_pkt =
            local.to_connection_packet(peer_conn.str_field("deviceId").unwrap_or(""), framing);
        stream
            .write_all(&Framing::Newline.encode(&conn_pkt.to_bytes()))
            .map_err(|e| format!("no se pudo enviar la identidad: {e}"))?;
        stream.flush().ok();

        // TLS: como aceptadores del TCP, somos el CLIENTE TLS.
        let pinned = peer_conn
            .str_field("deviceId")
            .and_then(|id| peers.fingerprint(id));
        let client_cfg = material.client_config(pinned.as_deref())?;
        let server_name = rustls::pki_types::ServerName::try_from(
            peer_conn
                .str_field("deviceId")
                .filter(|id| Identity::is_valid_device_id(id))
                .unwrap_or("kdeconnect")
                .to_string(),
        )
        .map_err(|e| format!("nombre de servidor inválido: {e}"))?;
        let mut conn = rustls::ClientConnection::new(client_cfg, server_name)
            .map_err(|e| format!("TLS: no se pudo crear la conexión: {e}"))?;
        complete_tls_io(&mut conn, &mut stream)?;
        // El certificado del peer (el del servidor TLS).
        let peer_cert_fingerprint = conn
            .peer_certificates()
            .and_then(|c| c.first())
            .map(|c| crate::selfsigned_cert::fingerprint_hex(c.as_ref()));
        let mut secure = Secure::Client(rustls::StreamOwned::new(conn, stream));

        let mut decoder = FrameDecoder::with_framing(framing);
        secure
            .write_all(&framing.encode(&local.to_identity_packet().to_bytes()))
            .map_err(|e| format!("no se pudo enviar la identidad cifrada: {e}"))?;
        let peer_raw = read_packet(&mut secure, &mut decoder)?;
        let peer_pkt =
            Packet::from_slice(&peer_raw).ok_or("la identidad cifrada del peer no es un paquete")?;
        let peer = Identity::from_packet(&peer_pkt)
            .ok_or("la identidad cifrada del peer no es válida")?;
        let trusted = peers.is_trusted(&peer.device_id);
        check_pin(peers, &peer.device_id, peer_cert_fingerprint.as_deref())?;

        Ok(Session {
            peer,
            trusted,
            framing,
            local: local.clone(),
            peer_cert_fingerprint,
            secure,
            decoder,
        })
    }

    /// Envía un paquete.
    pub fn send(&mut self, packet: &Packet) -> Result<(), String> {
        self.secure
            .write_all(&self.framing.encode(&packet.to_bytes()))
            .map_err(|e| format!("no se pudo enviar el paquete: {e}"))
    }

    /// Recibe un paquete.
    pub fn recv(&mut self) -> Result<Packet, String> {
        let raw = read_packet(&mut self.secure, &mut self.decoder)?;
        Packet::from_slice(&raw).ok_or_else(|| "el peer mandó un paquete inválido".to_string())
    }

    /// Hace un `ping` y devuelve el RTT (mide sólo el ida y vuelta del ping).
    pub fn ping(&mut self) -> Result<Duration, String> {
        let t0 = std::time::Instant::now();
        self.send(&Packet::new(ptype::PING, json!({ "message": "ping" })))?;
        loop {
            let p = self.recv()?;
            if p.ptype == ptype::PONG {
                return Ok(t0.elapsed());
            }
        }
    }

    // ── Empareado ─────────────────────────────────────────────────────────

    /// Envía al par una **petición de empareado** con PIN.
    ///
    /// El par la responde con `kdeconnect.pair` (accepted / denied).  El
    /// PIN es el que el usuario tiene que verificar en la otra pantalla.
    pub fn request_pairing(&mut self, pin: &str) -> Result<(), String> {
        let req = PairRequest {
            pair: true,
            pin: Some(pin.to_string()),
            timestamp: now_epoch(),
            device_name: Some(self.local.device_name.clone()),
            device_id: Some(self.local.device_id.clone()),
        };
        self.send(&req.to_packet())
    }

    /// Resuelve una **petición de empareado entrante**.
    ///
    /// * `Accept` ⇒ se empareja al par (se guarda su huella de certificado
    ///   como *pin*) y se le responde `pair: true` con el PIN devuelto;
    /// * `NeedsUserConfirmation` ⇒ **no** se responde todavía: falta el
    ///   "sí" del usuario (que es la frontera de seguridad);
    /// * cualquier otra ⇒ se responde `pair: false`.
    ///
    /// Devuelve la decisión que se tomó.
    pub fn handle_pair_request(
        &mut self,
        packet: &Packet,
        policy: &PairPolicy,
        user_confirmed: bool,
        peers: &mut PeerStore,
    ) -> Result<PairDecision, String> {
        let req = PairRequest::from_packet(packet).ok_or("no es un paquete de pareado")?;
        let decision = decide_pairing(&req, policy, user_confirmed, now_epoch());
        match decision {
            PairDecision::Accept => {
                let fingerprint = self
                    .peer_cert_fingerprint
                    .clone()
                    .ok_or("el peer no presentó certificado: no se puede emparejar")?;
                peers
                    .trust(PeerRecord {
                        device_id: self.peer.device_id.clone(),
                        device_name: self.peer.device_name.clone(),
                        cert_fingerprint: fingerprint,
                        paired_at: now_epoch(),
                        last_seen: now_epoch(),
                        trusted: true,
                    })
                    .map_err(|e| format!("no se pudo guardar el empareado: {e}"))?;
                self.trusted = true;
                // La respuesta **devuelve el PIN**: el que pidió el
                // emparejado ve que el otro lo leyó (y lo comprobó).
                let reply = PairRequest {
                    pair: true,
                    pin: req.pin.clone(),
                    timestamp: now_epoch(),
                    device_name: Some(self.local.device_name.clone()),
                    device_id: Some(self.local.device_id.clone()),
                };
                self.send(&reply.to_packet())?;
            }
            PairDecision::NeedsUserConfirmation => {
                // Sin confirmación humana no se responde: el par espera.
            }
            _ => {
                let reply = PairRequest {
                    pair: false,
                    pin: None,
                    timestamp: now_epoch(),
                    device_name: Some(self.local.device_name.clone()),
                    device_id: Some(self.local.device_id.clone()),
                };
                self.send(&reply.to_packet())?;
            }
        }
        Ok(decision)
    }

    /// Envía un `md.request` y espera su `md.response`.
    ///
    /// Devuelve `Err` si el peer no responde (p. ej. porque no está
    /// emparejado y descarta la petición).
    pub fn request(
        &mut self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<Value, String> {
        let mut b = Map::new();
        b.insert("method".to_string(), Value::String(method.to_string()));
        b.insert("path".to_string(), Value::String(path.to_string()));
        if let Some(body) = body {
            b.insert("body".to_string(), Value::String(body.to_string()));
        }
        let id = new_uuid();
        let pkt = Packet::with_id(&id, ptype::MD_REQUEST, Value::Object(b));
        self.send(&pkt)?;
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while std::time::Instant::now() < deadline {
            let p = self.recv()?;
            if p.ptype == ptype::MD_RESPONSE && p.id == id {
                return Ok(p.body);
            }
        }
        Err("el peer no respondió al md.request".into())
    }
}

/// Comprueba el *pin* del peer si ya está emparejado.
///
/// Es la comprobación **simétrica** del emparejado: en el rol de cliente
/// TLS el verificador de rustls ya la hizo, pero en el rol de servidor TLS
/// (el que inició la conexión) no había verificación: aquí se cierra.
fn check_pin(
    peers: &PeerStore,
    peer_id: &str,
    presented: Option<&str>,
) -> Result<(), String> {
    let Some(expected) = peers.fingerprint(peer_id) else {
        return Ok(());
    };
    match presented {
        Some(actual) if constant_time_eq(expected.as_bytes(), actual.as_bytes()) => Ok(()),
        Some(_) => Err("el certificado del peer no es el que se emparejó".into()),
        None => Err("el peer no presentó certificado: no se puede comprobar el pin".into()),
    }
}

/// Lee un paquete (con framing) del flujo TLS.
fn read_packet(secure: &mut Secure, decoder: &mut FrameDecoder) -> Result<Vec<u8>, String> {
    let mut buf = [0u8; 16 * 1024];
    let limite = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        let n = match secure.read(&mut buf) {
            Ok(n) => n,
            // MEDIDO el 2026-11-21 (test `dos_nodos_hacen_ping_por_el_canal_
            // cifrado`): con un timeout de lectura configurado, Rust devuelve
            // `WouldBlock` (os error 11) cuando todavia no han llegado bytes.
            // Antes eso se traducía a un error y el test fallaba segun el
            // ritmo (mas rapido en release que en debug).
            //
            // Ahora se ESPERA: un timeout a mitad de trama no mata la
            // sesion, se reintenta hasta el plazo.
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                if std::time::Instant::now() >= limite {
                    return Err("timeout esperando datos del peer".into());
                }
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            Err(e) => return Err(format!("error de lectura: {e}")),
        };
        if n == 0 {
            return Err("el peer cerró la conexión".into());
        }
        let frames = decoder
            .push(&buf[..n])
            .map_err(|e| format!("error de framing: {e}"))?;
        if let Some(f) = frames.into_iter().next() {
            return Ok(f);
        }
    }
}

/// Lee una línea terminada en `\n` (byte a byte, como hace el oficial).
///
/// Byte a byte a propósito: es el único modo de no dejar bytes en un búfer
/// intermedio, porque justo después el mismo socket pasa a TLS.
fn read_line_bounded(stream: &mut TcpStream, max: usize) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "el peer cerró antes de enviar la identidad",
            ));
        }
        if byte[0] == b'\n' {
            return Ok(out);
        }
        out.push(byte[0]);
        if out.len() > max {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "paquete de identidad demasiado largo",
            ));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Descubrimiento (UDP en difusión)
// ═══════════════════════════════════════════════════════════════════════════

/// Peer visto por el descubrimiento.
#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    /// Identidad del peer.
    pub identity: Identity,
    /// Dirección desde la que llegó el datagram.
    pub addr: SocketAddr,
    /// Puerto UDP por el que se oyó (1716 oficial / 1776 enjambre).
    pub udp_port: u16,
    /// Epoch de la recepción.
    pub seen_at: u64,
}

/// Tipo de paquete que se manda en cada puerto de descubrimiento.
///
/// En 1716 (oficial) va `kdeconnect.identity`; en 1776 (enjambre) va
/// `kdeconnect.discovery`.  El cuerpo es idéntico en los dos casos.
pub fn discovery_type_for_port(port: u16) -> &'static str {
    if port == UDP_PORT_OFFICIAL {
        ptype::IDENTITY
    } else {
        ptype::DISCOVERY
    }
}

/// Anuncia la identidad de este nodo en difusión (UDP).
///
/// Devuelve cuántos datagrams se han enviado.
pub fn broadcast_discovery(
    sock: &UdpSocket,
    identity: &Identity,
    tcp_port: u16,
    ports: &[u16],
) -> usize {
    let mut n = 0;
    for port in ports {
        let mut id = identity.clone();
        id.tcp_port = tcp_port;
        let payload = id.to_discovery_packet(discovery_type_for_port(*port)).to_bytes();
        let targets = [
            // Difusión global.
            SocketAddr::from((Ipv4Addr::BROADCAST, *port)),
            // Loopback: sirve para que dos procesos del **mismo** host se
            // vean (y para probar sin segunda máquina).  Es inocuo: un nodo
            // se filtra a sí mismo por `deviceId`.
            SocketAddr::from((Ipv4Addr::LOCALHOST, *port)),
        ];
        for t in targets {
            if sock.send_to(&payload, t).is_ok() {
                n += 1;
            }
        }
    }
    n
}

/// Escucha paquetes de descubrimiento durante `timeout`.
///
/// Escucha en los puertos indicados (1716 y 1776 por defecto) y devuelve
/// los peers vistos (sin repetir el mismo `deviceId`).
pub fn listen_discovery(timeout: Duration, ports: &[u16]) -> Vec<DiscoveredPeer> {
    let mut out: Vec<DiscoveredPeer> = Vec::new();
    let deadline = std::time::Instant::now() + timeout;
    let mut socks: Vec<(u16, UdpSocket)> = Vec::new();
    for p in ports {
        if let Ok(s) = UdpSocket::bind(("0.0.0.0", *p)) {
            let _ = s.set_read_timeout(Some(Duration::from_millis(250)));
            let _ = s.set_broadcast(true);
            socks.push((*p, s));
        }
    }
    let mut buf = [0u8; 8192];
    while std::time::Instant::now() < deadline {
        for (port, sock) in socks.iter() {
            match sock.recv_from(&mut buf) {
                Ok((n, from)) => {
                    let Some(pkt) = Packet::from_slice(&buf[..n]) else {
                        continue;
                    };
                    let Some(identity) = Identity::from_packet(&pkt) else {
                        continue;
                    };
                    if out.iter().any(|d| d.identity.device_id == identity.device_id) {
                        continue;
                    }
                    out.push(DiscoveredPeer {
                        identity,
                        addr: from,
                        udp_port: *port,
                        seen_at: now_epoch(),
                    });
                }
                Err(_) => continue,
            }
        }
    }
    out
}

/// Epoch actual en segundos.
pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn identidad_de_prueba() -> Identity {
        Identity {
            device_id: new_device_id(),
            device_name: "movil-de-prueba".into(),
            device_type: "phone".into(),
            protocol_version: PROTOCOL_VERSION,
            tcp_port: TCP_PORT_DEFAULT,
            incoming_capabilities: default_capabilities(),
            outgoing_capabilities: default_capabilities(),
            app_name: APP_NAME.into(),
        }
    }

    // ── Paquete ────────────────────────────────────────────────────────────

    #[test]
    fn el_paquete_seria_como_kde_connect() {
        let p = Packet::new(ptype::PAIR, json!({ "pair": true, "timestamp": 42 }));
        let v = p.to_json();
        // Esquema oficial: id + type + body.
        assert!(v.get("id").is_some());
        assert_eq!(v["type"], json!(ptype::PAIR));
        assert_eq!(v["body"]["pair"], json!(true));
        assert_eq!(v["body"]["timestamp"], json!(42));
        // El `id` es un UUID (string).
        let id = v["id"].as_str().unwrap();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn el_paquete_ida_y_vuelve() {
        let p = Packet::new(ptype::PING, json!({ "message": "hola" }));
        let bytes = p.to_bytes();
        let back = Packet::from_slice(&bytes).expect("debe releerse");
        assert_eq!(back.ptype, ptype::PING);
        assert_eq!(back.id, p.id);
        assert_eq!(back.body["message"], json!("hola"));
    }

    #[test]
    fn el_id_numerico_del_oficial_se_acepta() {
        // El oficial manda el `id` como timestamp en milisegundos.
        let v = json!({ "id": 1599051054076u64, "type": "kdeconnect.identity", "body": {} });
        let p = Packet::from_json(&v).expect("debe parsear");
        assert_eq!(p.id, "1599051054076");
        assert_eq!(p.ptype, ptype::IDENTITY);
    }

    #[test]
    fn un_json_sin_type_no_es_un_paquete() {
        assert!(Packet::from_json(&json!({ "id": "x", "body": {} })).is_none());
        assert!(Packet::from_json(&json!({ "type": "", "body": {} })).is_none());
    }

    // ── Framing ────────────────────────────────────────────────────────────

    #[test]
    fn el_framing_oficial_es_json_mas_nueva_linea() {
        let f = Framing::Newline;
        let out = f.encode(b"{\"a\":1}");
        assert_eq!(out, b"{\"a\":1}\n".to_vec());
    }

    #[test]
    fn el_framing_de_longitud_es_dos_bytes_big_endian() {
        let f = Framing::LengthPrefixed;
        let out = f.encode(b"hola");
        assert_eq!(&out[..2], &[0x00, 0x04]);
        assert_eq!(&out[2..], b"hola");
    }

    #[test]
    fn el_framing_ida_y_vuelve_troceado_en_bytes_arbitrarios() {
        // Se trocea el flujo en trozos de 1, 3, 7 y 1000 bytes: el
        // protocolo real entrega lo que puede en cada `read()`.
        for framing in [Framing::Newline, Framing::LengthPrefixed] {
            let mut esperado: Vec<Vec<u8>> = Vec::new();
            let mut flujo: Vec<u8> = Vec::new();
            for i in 0..5u8 {
                let payload = format!("{{\"n\":{i},\"texto\":\"ñáéí\"}}").into_bytes();
                esperado.push(payload.clone());
                flujo.extend_from_slice(&framing.encode(&payload));
            }
            for trozo in [1usize, 3, 7, 1000] {
                let mut dec = FrameDecoder::with_framing(framing);
                let mut recibido: Vec<Vec<u8>> = Vec::new();
                for bloque in flujo.chunks(trozo) {
                    let frames = dec.push(bloque).expect("no debe fallar");
                    recibido.extend(frames);
                }
                assert_eq!(recibido, esperado, "framing {framing:?} troceado en {trozo}");
                assert_eq!(dec.pending(), 0);
            }
        }
    }

    #[test]
    fn el_framing_se_autodetecta() {
        let mut dec = FrameDecoder::new();
        assert!(dec.framing().is_none());
        // Un paquete JSON empieza por '{' -> newline.
        let frames = dec.push(b"{\"a\":1}\n").unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(dec.framing(), Some(Framing::Newline));

        let mut dec2 = FrameDecoder::new();
        let frames = dec2.push(&Framing::LengthPrefixed.encode(b"xyz")).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(dec2.framing(), Some(Framing::LengthPrefixed));
    }

    #[test]
    fn un_frame_demasiado_grande_se_rechaza() {
        // Tope de 32 bytes: el prefijo declara 64 y no cabe.
        let mut dec = FrameDecoder::with_max_packet(32);
        let bytes = vec![0x00, 0x40];
        assert!(matches!(dec.push(&bytes), Err(FrameError::TooLarge(_))));

        // Con el framing oficial, lo que pasa es que el búfer crece sin
        // encontrar el '\n'.
        let mut dec2 = FrameDecoder::with_max_packet(32);
        assert!(matches!(
            dec2.push(&vec![b'x'; 64]),
            Err(FrameError::TooLarge(_))
        ));
    }

    // ── Descubrimiento ─────────────────────────────────────────────────────

    #[test]
    fn el_paquete_de_descubrimiento_tiene_los_campos_obligatorios() {
        let id = identidad_de_prueba();
        let p = id.to_discovery_packet(discovery_type_for_port(UDP_PORT_OFFICIAL));
        let b = &p.body;
        for campo in [
            "deviceId",
            "deviceName",
            "deviceType",
            "protocolVersion",
            "incomingCapabilities",
            "outgoingCapabilities",
            "tcpPort",
            "appName",
        ] {
            assert!(b.get(campo).is_some(), "falta el campo {campo} en el descubrimiento");
        }
        assert_eq!(p.ptype, ptype::IDENTITY);
        assert_eq!(b["deviceId"], json!(id.device_id));
        assert_eq!(b["deviceName"], json!(id.device_name));
        assert_eq!(b["protocolVersion"], json!(PROTOCOL_VERSION));
        assert_eq!(b["tcpPort"], json!(TCP_PORT_DEFAULT));
        assert!(b["incomingCapabilities"].as_array().unwrap().len() >= 3);
        assert!(b["outgoingCapabilities"].as_array().unwrap().len() >= 3);
        // Y el tipo del enjambre es el otro.
        let mesh = id.to_discovery_packet(discovery_type_for_port(UDP_PORT_MESH));
        assert_eq!(mesh.ptype, ptype::DISCOVERY);
        assert_eq!(mesh.body["deviceId"], b["deviceId"]);
    }

    #[test]
    fn la_identidad_ida_y_vuelve() {
        let id = identidad_de_prueba();
        let p = id.to_identity_packet();
        let back = Identity::from_packet(&p).expect("debe releerse");
        assert_eq!(back.device_id, id.device_id);
        assert_eq!(back.device_name, id.device_name);
        assert_eq!(back.device_type, id.device_type);
        assert_eq!(back.protocol_version, id.protocol_version);
        assert_eq!(back.tcp_port, id.tcp_port);
        assert_eq!(back.incoming_capabilities, id.incoming_capabilities);
    }

    #[test]
    fn el_device_id_sigue_el_patrono_del_oficial() {
        let id = new_device_id();
        assert_eq!(id.len(), 32);
        assert!(Identity::is_valid_device_id(&id));
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        // Dos ids nunca son iguales.
        assert_ne!(id, new_device_id());
    }

    #[test]
    fn el_nombre_se_sanea_como_el_oficial() {
        assert_eq!(Identity::sanitize_device_name("a\"b'c;d:e.f!g?h(i)j[k]l<m>n"), "abcdefghijklmn");
        assert_eq!(Identity::sanitize_device_name(&"x".repeat(50)).len(), 32);
    }

    #[test]
    fn una_identidad_con_id_invalido_se_rechaza() {
        let mut p = identidad_de_prueba().to_identity_packet();
        p.body["deviceId"] = json!("corto");
        assert!(Identity::from_packet(&p).is_none());
    }

    // ── Pareado (PIN) ──────────────────────────────────────────────────────

    fn peticion(pin: Option<&str>) -> PairRequest {
        PairRequest {
            pair: true,
            pin: pin.map(|s| s.to_string()),
            timestamp: 1_000,
            device_name: Some("otro-nodo".into()),
            device_id: Some(new_device_id()),
        }
    }

    #[test]
    fn el_pin_correcto_se_acepta() {
        let policy = PairPolicy {
            require_pin: true,
            expected_pin: Some("48219037".into()),
            require_user_confirmation: true,
        };
        let d = decide_pairing(&peticion(Some("48219037")), &policy, true, 1_000);
        assert_eq!(d, PairDecision::Accept);
        assert!(d.is_accept());
    }

    #[test]
    fn el_pin_incorrecto_se_rechaza() {
        let policy = PairPolicy {
            require_pin: true,
            expected_pin: Some("48219037".into()),
            require_user_confirmation: true,
        };
        let d = decide_pairing(&peticion(Some("00000000")), &policy, true, 1_000);
        assert_eq!(d, PairDecision::WrongPin);
        assert!(!d.is_accept());
    }

    #[test]
    fn un_paquete_sin_pin_se_rechaza() {
        let policy = PairPolicy {
            require_pin: true,
            expected_pin: Some("48219037".into()),
            require_user_confirmation: true,
        };
        let d = decide_pairing(&peticion(None), &policy, true, 1_000);
        assert_eq!(d, PairDecision::MissingPin);
        assert!(!d.is_accept());
    }

    #[test]
    fn sin_confirmacion_del_usuario_no_se_empareja() {
        let policy = PairPolicy {
            require_pin: true,
            expected_pin: Some("48219037".into()),
            require_user_confirmation: true,
        };
        let d = decide_pairing(&peticion(Some("48219037")), &policy, false, 1_000);
        assert_eq!(d, PairDecision::NeedsUserConfirmation);
        assert!(!d.is_accept());
    }

    #[test]
    fn una_peticion_vieja_se_rechaza() {
        let policy = PairPolicy::default();
        let ahora = 1_000 + PAIR_REQUEST_TTL_S + 60;
        let d = decide_pairing(&peticion(Some("48219037")), &policy, true, ahora);
        assert_eq!(d, PairDecision::Expired);
    }

    #[test]
    fn pair_false_es_un_desempareado() {
        let policy = PairPolicy::default();
        let mut req = peticion(Some("48219037"));
        req.pair = false;
        let d = decide_pairing(&req, &policy, true, 1_000);
        assert_eq!(d, PairDecision::Deny);
    }

    #[test]
    fn el_paquete_de_pareado_ida_y_vuelve_con_el_pin() {
        let req = peticion(Some("48219037"));
        let pkt = req.to_packet();
        assert_eq!(pkt.ptype, ptype::PAIR);
        assert_eq!(pkt.body["pair"], json!(true));
        assert_eq!(pkt.body["pin"], json!("48219037"));
        let back = PairRequest::from_packet(&pkt).expect("debe releerse");
        assert_eq!(back, req);
    }

    #[test]
    fn la_comparacion_de_pin_es_de_tiempo_constante() {
        assert!(verify_pin("48219037", "48219037"));
        assert!(!verify_pin("48219037", "48219038"));
        assert!(!verify_pin("48219037", "4821903"));
        assert!(!verify_pin("48219037", ""));
        // El digest con sal no es el PIN en claro.
        let d = pin_digest("48219037", b"sal");
        assert_ne!(d[..], b"48219037"[..]);
        assert_eq!(d, pin_digest("48219037", b"sal"));
        assert_ne!(d, pin_digest("48219037", b"otra"));
    }

    #[test]
    fn el_pin_generado_son_ocho_digitos() {
        for _ in 0..20 {
            let pin = generate_pin();
            assert_eq!(pin.len(), PIN_DIGITS);
            assert!(pin.chars().all(|c| c.is_ascii_digit()), "pin no numérico: {pin}");
        }
    }

    // ── Canal de control ───────────────────────────────────────────────────

    #[test]
    fn el_ping_devuelve_un_pong() {
        let id = identidad_de_prueba();
        let peers = PeerStore::open("/tmp/kde-test-inexistente");
        let ctx = ControlContext {
            identity: &id,
            peers: &peers,
            local_daemon: None,
            peer_id: "otro".into(),
            peer_trusted: true,
        };
        let resp = handle_control(
            &Packet::new(ptype::PING, json!({ "message": "hola" })),
            &ctx,
        )
        .expect("debe responder");
        assert_eq!(resp.ptype, ptype::PONG);
        assert_eq!(resp.body["message"], json!("hola"));
    }

    #[test]
    fn el_echo_devuelve_el_mismo_texto() {
        let id = identidad_de_prueba();
        let peers = PeerStore::open("/tmp/kde-test-inexistente");
        let ctx = ControlContext {
            identity: &id,
            peers: &peers,
            local_daemon: None,
            peer_id: "otro".into(),
            peer_trusted: true,
        };
        let resp = handle_control(
            &Packet::new(ptype::ECHO, json!({ "message": "abc" })),
            &ctx,
        )
        .expect("debe responder");
        assert_eq!(resp.ptype, ptype::ECHO);
        assert_eq!(resp.body["message"], json!("abc"));
    }

    #[test]
    fn un_md_request_de_un_peer_no_emparejado_se_descarta() {
        // FRONTERA DE SEGURIDAD: sin emparejar, el peer no puede pedir nada
        // (aunque el daemon local esté disponible).
        let id = identidad_de_prueba();
        let peers = PeerStore::open("/tmp/kde-test-inexistente");
        let daemon = LocalDaemon::default();
        let ctx = ControlContext {
            identity: &id,
            peers: &peers,
            local_daemon: Some(&daemon),
            peer_id: "desconocido".into(),
            peer_trusted: false,
        };
        let resp = handle_control(
            &Packet::new(ptype::MD_REQUEST, json!({ "method": "GET", "path": "/api/status" })),
            &ctx,
        );
        assert!(resp.is_none(), "un peer sin emparejar no obtiene respuesta");
    }

    #[test]
    fn un_tipo_desconocido_se_descarta() {
        let id = identidad_de_prueba();
        let peers = PeerStore::open("/tmp/kde-test-inexistente");
        let ctx = ControlContext {
            identity: &id,
            peers: &peers,
            local_daemon: None,
            peer_id: "otro".into(),
            peer_trusted: true,
        };
        assert!(handle_control(&Packet::new("kdeconnect.runcommand", json!({})), &ctx).is_none());
    }

    #[test]
    fn el_md_request_rechaza_rutas_absolutas() {
        let d = LocalDaemon::default();
        assert!(d.forward("GET", "http://evil.example/", None).is_err());
        assert!(d.forward("GET", "/api/status\n", None).is_err());
        assert!(d.forward("TRACE", "/api/status", None).is_err());
    }

    // ── Almacén de peers ───────────────────────────────────────────────────

    #[test]
    fn el_almacen_de_peers_guarda_y_revisa() {
        let dir = std::env::temp_dir().join(format!("md-kde-peers-{}", std::process::id()));
        let state = dir.to_string_lossy().to_string();
        let _ = std::fs::remove_dir_all(&dir);

        let mut store = PeerStore::open(&state);
        store.load().expect("carga vacía");
        let id = new_device_id();
        assert!(!store.is_trusted(&id));
        store
            .trust(PeerRecord {
                device_id: id.clone(),
                device_name: "pc".into(),
                cert_fingerprint: "aa:bb".into(),
                paired_at: 1,
                last_seen: 1,
                trusted: true,
            })
            .expect("guarda");
        assert!(store.is_trusted(&id));
        assert_eq!(store.fingerprint(&id).as_deref(), Some("aa:bb"));

        // Recargar desde disco conserva la confianza.
        let mut store2 = PeerStore::open(&state);
        store2.load().expect("carga");
        assert!(store2.is_trusted(&id));

        // Y revocar la quita.
        assert!(store2.revoke(&id).expect("revoca"));
        assert!(!store2.is_trusted(&id));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Sesión completa (TCP + TLS + canal de control) ────────────────────

    #[test]
    fn dos_nodos_hacen_ping_por_el_canal_cifrado() {
        // Prueba de extremo a extremo en loopback: descubrimiento no hace
        // falta, pero sí el handshake completo (identidad en claro → TLS →
        // identidad cifrada → pin/pong) y el `md.request`.
        use std::net::TcpListener;

        // MEDIDO (2026-11-26, CI): este test era **flaky** (fallaba a
        // veces en el CI y pasaba en local).  Dos causas:
        //   a) el directorio temporal dépendía SÓLO del PID ⇒ dos ejecuciones
        //      (o dos binarios de test a la vez) se pisaban el estado;
        //   b) el timeout de la sesión eran 5 s,too justos con la CI cargada
        //      (handshake TLS + ping) ⇒ `expect("ping")` reventaba.
        // Ahora el directorio es único por ejecución y el timeout es amplio
        // (el RTT medido sigue teniendo que ser < 5 s).
        let unico = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir_a = std::env::temp_dir().join(format!("md-kde-a-{}-{unico}", std::process::id()));
        let dir_b = std::env::temp_dir().join(format!("md-kde-b-{}-{unico}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
        let state_a = dir_a.to_string_lossy().to_string();
        let state_b = dir_b.to_string_lossy().to_string();

        let id_a = DeviceIdentity::load_or_create(&state_a, "nodo-a").expect("identidad A");
        let id_b = DeviceIdentity::load_or_create(&state_b, "nodo-b").expect("identidad B");
        let mat_a = TlsMaterial::load_or_create(&state_a, &id_a.device_id).expect("cert A");
        let mat_b = TlsMaterial::load_or_create(&state_b, &id_b.device_id).expect("cert B");

        // Cada lado confía en el otro (con su pin real).
        let mut store_a = PeerStore::open(&state_a);
        store_a
            .trust(PeerRecord {
                device_id: id_b.device_id.clone(),
                device_name: id_b.device_name.clone(),
                cert_fingerprint: mat_b.fingerprint.clone(),
                paired_at: 0,
                last_seen: 0,
                trusted: true,
            })
            .expect("trust A");
        let mut store_b = PeerStore::open(&state_b);
        store_b
            .trust(PeerRecord {
                device_id: id_a.device_id.clone(),
                device_name: id_a.device_name.clone(),
                cert_fingerprint: mat_a.fingerprint.clone(),
                paired_at: 0,
                last_seen: 0,
                trusted: true,
            })
            .expect("trust B");

        let identity_a = id_a.to_identity("desktop", 0);
        let identity_b = id_b.to_identity("desktop", 0);
        let cfg = SessionConfig {
            framing: Some(Framing::LengthPrefixed),
            // MEDIDO (2026-11-26, CI): 5 s era demasiado justo con la CI
            // cargada (el test fallaba a veces).  El RTT medido sigue
            // teniendo que ser < 5 s (eso lo comprueba el assert de abajo).
            timeout: Duration::from_secs(20),
        };

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");

        // El que acepta la conexión es el cliente TLS; el que inicia es el
        // servidor TLS (roles del protocolo KDE Connect).
        let cfg_hilo = cfg.clone();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut sess = Session::accept(stream, &identity_a, &mat_a, &store_a, &cfg_hilo)
                .expect("sesion A");
            assert!(sess.trusted);
            assert_eq!(sess.framing, Framing::LengthPrefixed);
            // Contesta al ping y al md.request.
            loop {
                // Cuando B cierra la sesión, `recv` falla y el hilo sale.
                let Ok(p) = sess.recv() else { break };
                match p.ptype.as_str() {
                    ptype::PING => {
                        let resp = handle_control(
                            &p,
                            &ControlContext {
                                identity: &sess.local,
                                peers: &store_a,
                                local_daemon: Some(&LocalDaemon::default()),
                                peer_id: sess.peer.device_id.clone(),
                                peer_trusted: true,
                            },
                        );
                        if let Some(r) = resp {
                            sess.send(&r).expect("pong");
                        }
                    }
                    ptype::MD_REQUEST => {
                        let resp = handle_control(
                            &p,
                            &ControlContext {
                                identity: &sess.local,
                                peers: &store_a,
                                local_daemon: Some(&LocalDaemon::default()),
                                peer_id: sess.peer.device_id.clone(),
                                peer_trusted: true,
                            },
                        );
                        if let Some(r) = resp {
                            sess.send(&r).expect("md.response");
                        }
                    }
                    _ => {}
                }
            }
        });

        let mut sess = Session::connect(
            addr,
            &id_a.device_id,
            &identity_b,
            &mat_b,
            &store_b,
            &cfg,
        )
        .expect("sesion B");
        assert!(sess.trusted, "B debe ver a A como emparejado");
        assert_eq!(sess.peer.device_id, id_a.device_id);

        let rtt = sess.ping().expect("ping");
        assert!(rtt.as_secs_f64() < 5.0, "RTT razonable: {rtt:?}");

        // El md.request llega al otro nodo y vuelve la respuesta (el daemon
        // local no está en el test, así que el estado es 0 con el error).
        let resp = sess.request("GET", "/api/status", None).expect("md.response");
        assert_eq!(resp["status"], json!(0));

        // Cerrar la sesión para que el hilo A no se quede esperando.
        drop(sess);
        handle.join().expect("hilo A");
        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
    }

    // ── TLS ────────────────────────────────────────────────────────────────

    #[test]
    fn el_certificado_propio_lo_acepta_rustls() {
        // Si rustls acepta el certificado (SPKI correcto, DER válido), el
        // canal TLS puede arrancar.
        let cert = crate::selfsigned_cert::generate("nodo-test", 30).expect("cert");
        let material = TlsMaterial {
            cert_der: cert.der,
            key_pkcs8: cert.key_pkcs8,
            fingerprint: cert.fingerprint,
        };
        let cfg = material.server_config().expect("rustls acepta el cert");
        assert!(cfg.alpn_protocols.is_empty());
        // El cliente también se puede construir (con y sin pin).
        assert!(material.client_config(None).is_ok());
        assert!(material.client_config(Some("00")).is_ok());
    }
}
