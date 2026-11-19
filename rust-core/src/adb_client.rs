//! Cliente ADB en Rust puro: habla el protocolo deADB directamente por
//! socket, sin depender del binario `adb` ni de platform-tools.
//!
//! Por qué
//! -------
//! El binario `megadaemon-adb` (src/bin/megadaemon-adb.rs) envuelve el
//! binario `adb` externo: en el móvil no existe (`which adb` -> vacío), así que
//! el único camino que funciona ahí es hablar el protocolo a mano contra
//! `adbd`, que ya expone el puerto TCP 5555 (`service.adb.tcp.port=5555`,
//! `init.svc.adbd=running`).
//!
//! Protocolo (ADB, "ADB protocol v1"): cada mensaje es una cabecera de 24
//! bytes en little-endian seguida del payload:
//!
//! ```text
//! offset 0  u32  command      "CNXN" | "AUTH" | "OPEN" | "OKAY" | "CLSE" | "WRTE"
//! offset 4  u32  arg0         (stream id / versión / longitud)
//! offset 8  u32  arg1
//! offset 12 u32  data_length
//! offset 16 u32  data_crc32   crc32 IEEE del payload
//! offset 20 u32  magic        command ^ 0xFFFFFFFF
//! ```
//!
//! Dos transportes:
//!   * **Servidor ADB** (`127.0.0.1:5037`): `host:devices-l`,
//!     `host:transport:<serial>:<cmd>`.
//!   * **adbd directo** (`<ip>:5555`): handshake `CNXN` + `OPEN shell:<cmd>`.
//!     Es el único modo utilizable desde el propio móvil.
//!
//! Todos los bloqueos tienen `timeout` y toda la salida tiene tope de bytes:
//! un dispositivo colgado no puede comerse un hilo ni la memoria del daemon.

use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use rsa::pkcs1v15::SigningKey as RsaSigningKey;
use rsa::signature::{SignatureEncoding, Signer};
use rsa::RsaPrivateKey;
use sha1::Sha1;

pub const CMD_CNXN: u32 = u32::from_le_bytes(*b"CNXN");
pub const CMD_AUTH: u32 = u32::from_le_bytes(*b"AUTH");
pub const CMD_OPEN: u32 = u32::from_le_bytes(*b"OPEN");
pub const CMD_OKAY: u32 = u32::from_le_bytes(*b"OKAY");
pub const CMD_CLSE: u32 = u32::from_le_bytes(*b"CLSE");
pub const CMD_WRTE: u32 = u32::from_le_bytes(*b"WRTE");
pub const CMD_SYNC: u32 = u32::from_le_bytes(*b"SYNC");

/// Versión del protocolo que anunciamos (1.0).
pub const ADB_VERSION: u32 = 0x0100_0000;
/// Tamaño máximo de payload que declaramos (4096 es el valor de AOSP).
pub const MAX_PAYLOAD: u32 = 4096;
/// Bytes de cabecera de cada mensaje.
pub const HEADER_LEN: usize = 24;
/// Tope por defecto de salida de un comando (1 MiB).
pub const DEFAULT_MAX_BYTES: usize = 1024 * 1024;
/// Banner que identifica al host ante `adbd`.
pub const HOST_BANNER: &str =
    "host::features=shell_v2,cmd,stat_v2,abb_exec,fixed_push_mkdir,sendrecv_v2,brotli_v2,openssl_v2";

// --- Protocolo SYNC de ADB (servicio `sync:`): push/pull de ficheros ---
/// Petición de envío de fichero al dispositivo.
pub const SYNC_SEND: u32 = u32::from_le_bytes(*b"SEND");
/// Petición de recepción de fichero del dispositivo.
pub const SYNC_RECV: u32 = u32::from_le_bytes(*b"RECV");
/// Fin de una transferencia (lleva el mtime).
pub const SYNC_DONE: u32 = u32::from_le_bytes(*b"DONE");
/// Bloque de datos.
pub const SYNC_DATA: u32 = u32::from_le_bytes(*b"DATA");
/// Respuesta correcta.
pub const SYNC_OKAY: u32 = u32::from_le_bytes(*b"OKAY");
/// Respuesta de error (lleva longitud + mensaje).
pub const SYNC_FAIL: u32 = u32::from_le_bytes(*b"FAIL");
/// Tamaño máximo de bloque en una transferencia.
pub const SYNC_CHUNK: usize = 64 * 1024;

/// Error del cliente ADB (con mensaje legible, sin secretos).
#[derive(Debug)]
pub enum AdbError {
    Io(io::Error),
    Protocol(String),
    Timeout(String),
    TooLarge(String),
}

impl std::fmt::Display for AdbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdbError::Io(e) => write!(f, "io: {e}"),
            AdbError::Protocol(s) => write!(f, "protocolo: {s}"),
            AdbError::Timeout(s) => write!(f, "timeout: {s}"),
            AdbError::TooLarge(s) => write!(f, "demasiado grande: {s}"),
        }
    }
}

impl std::error::Error for AdbError {}

impl From<io::Error> for AdbError {
    fn from(e: io::Error) -> Self {
        AdbError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, AdbError>;

/// Cómo se hashea el token `AUTH` antes de firmarlo.
///
/// AOSP genera el token con bytes aleatorios y lo NUL-termina, y tanto `adbd`
/// (`EVP_DigestVerifyUpdate(ctx, token, strlen(token))`) como platform-tools
/// (`EVP_DigestSignUpdate`) lo hashean con **semántica C**: hasta el primer
/// NUL.  Hay implementaciones que hashean los 20 bytes completos, así que el
/// cliente prueba las dos variantes: si el dispositivo rechaza la primera
/// firma, reintenta el handshake con la segunda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenHash {
    /// Semántica C: se hashea hasta el primer NUL (comportamiento de AOSP).
    CStr,
    /// Se hashean los bytes tal cual llegan.
    Full,
}

/// Clave de host ADB (RSA-2048 en PEM).
///
/// `adbd` con `ro.adb.secure=1` manda un token `AUTH` de 20 bytes y espera
/// una firma RSASSA-PKCS1-v1_5 sobre SHA-1 de ese token.  Sin una clave
/// registrada en `/data/misc/adb/adb_keys` el dispositivo se queda en
/// `unauthorized` para siempre.  Por eso la clave **nunca** está en el repo:
/// vive en `<state_dir>/adb_host_key.pem` (0600) o en el directorio privado de
/// la app en el móvil; sólo su parte pública se registra en el dispositivo.
#[derive(Clone)]
pub struct HostKey {
    key: RsaPrivateKey,
}

impl HostKey {
    /// Carga una clave de host desde PEM (PKCS#1 `RSA PRIVATE KEY` o PKCS#8
    /// `PRIVATE KEY`).
    pub fn from_pem(text: &str) -> Result<HostKey> {
        use rsa::pkcs1::DecodeRsaPrivateKey;
        use rsa::pkcs8::DecodePrivateKey;
        let key = RsaPrivateKey::from_pkcs1_pem(text)
            .or_else(|_| RsaPrivateKey::from_pkcs8_pem(text))
            .map_err(|e| AdbError::Protocol(format!("clave de host ADB ilegible: {e}")))?;
        Ok(HostKey { key })
    }

    /// Carga la clave de host de un fichero PEM.
    pub fn from_pem_file(path: &std::path::Path) -> Result<HostKey> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            AdbError::Io(io::Error::new(e.kind(), format!("{}: {e}", path.display())))
        })?;
        HostKey::from_pem(&text)
    }

    /// Firma el token `AUTH` (RSASSA-PKCS1-v1_5 sobre SHA-1), exactamente
    /// como hace `EVP_DigestSign` con `EVP_sha1()` en AOSP.
    pub fn sign_token(&self, token: &[u8], mode: TokenHash) -> Result<Vec<u8>> {
        let msg: &[u8] = match mode {
            TokenHash::CStr => {
                let end = token.iter().position(|&b| b == 0).unwrap_or(token.len());
                if end == 0 {
                    // Token con NUL al inicio: AOSP firmaría la cadena vacía,
                    // pero un DigestInfo degenerado no sirve.  Se firma al
                    // menos 1 byte.
                    &token[..token.len().min(1)]
                } else {
                    &token[..end]
                }
            }
            TokenHash::Full => token,
        };
        let signer = RsaSigningKey::<Sha1>::new(self.key.clone());
        let sig = signer
            .sign(msg)
            .map_err(|e| AdbError::Protocol(format!("fallo al firmar el token AUTH: {e}")))?;
        Ok(sig.to_vec())
    }

    /// Parte pública en base64, tal y como va en `/data/misc/adb/adb_keys`
    /// (`usuario@host <base64 SPKI>`).
    pub fn public_key_b64(&self) -> Result<String> {
        use rsa::pkcs8::EncodePublicKey;
        let der = self
            .key
            .to_public_key()
            .to_public_key_der()
            .map_err(|e| AdbError::Protocol(format!("no se pudo derivar la clave pública: {e}")))?;
        Ok(base64ct::Base64::encode_string(der.as_bytes()))
    }

    /// Tamaño del módulo en bits (2048 de serie).
    pub fn bits(&self) -> usize {
        self.key.n().bits()
    }
}

/// Rutas donde puede estar la clave de host ADB, en orden de prioridad.
pub fn host_key_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(p) = std::env::var("MEGADAEMON_ADB_KEY") {
        if !p.is_empty() {
            v.push(PathBuf::from(p));
        }
    }
    v.push(PathBuf::from(format!(
        "{}/adb_host_key.pem",
        crate::cockpit_server::state_dir()
    )));
    v.push(PathBuf::from("/data/local/tmp/megadaemon/adb_host_key.pem"));
    v.push(PathBuf::from("/data/adb/megadaemon/adb_host_key.pem"));
    v
}

/// Ruta de la clave de host que se usaría (o se usaría si existiera).
pub fn active_host_key_path() -> Option<PathBuf> {
    host_key_candidates().into_iter().find(|p| p.exists())
}

/// Carga (y cachea) la clave de host ADB.  `None` si no hay ninguna.
pub fn host_key() -> Option<HostKey> {
    static KEY: OnceLock<Option<HostKey>> = OnceLock::new();
    KEY.get_or_init(|| {
        for p in host_key_candidates() {
            if p.exists() {
                if let Ok(k) = HostKey::from_pem_file(&p) {
                    return Some(k);
                }
            }
        }
        None
    })
    .clone()
}

/// CRC-32 (IEEE 802.3, el mismo que `zlib.crc32`) con tabla compartida.
fn crc32_table() -> &'static [u32; 256] {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *e = c;
        }
        t
    })
}

/// CRC-32 del payload de un mensaje ADB.
pub fn crc32(data: &[u8]) -> u32 {
    let table = crc32_table();
    let mut c: u32 = 0xFFFF_FFFF;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Mensaje ADB: cabecera + payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub command: u32,
    pub arg0: u32,
    pub arg1: u32,
    pub payload: Vec<u8>,
}

impl Message {
    pub fn new(command: u32, arg0: u32, arg1: u32, payload: Vec<u8>) -> Self {
        Self { command, arg0, arg1, payload }
    }

    /// Serializa el mensaje (cabecera + payload).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.payload.len());
        out.extend_from_slice(&self.command.to_le_bytes());
        out.extend_from_slice(&self.arg0.to_le_bytes());
        out.extend_from_slice(&self.arg1.to_le_bytes());
        out.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&crc32(&self.payload).to_le_bytes());
        out.extend_from_slice(&(self.command ^ 0xFFFF_FFFF).to_le_bytes());
        out.extend_from_slice(&self.payload);
        out
    }

    /// Interpreta un buffer con cabecera + payload completos.
    pub fn decode(buf: &[u8]) -> Result<Message> {
        if buf.len() < HEADER_LEN {
            return Err(AdbError::Protocol(format!(
                "mensaje truncado: {} bytes < {HEADER_LEN}",
                buf.len()
            )));
        }
        let u32at = |o: usize| {
            u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]])
        };
        let command = u32at(0);
        let arg0 = u32at(4);
        let arg1 = u32at(8);
        let len = u32at(12) as usize;
        let crc = u32at(16);
        let magic = u32at(20);
        if magic != (command ^ 0xFFFF_FFFF) {
            return Err(AdbError::Protocol("magic incorrecto".into()));
        }
        if buf.len() < HEADER_LEN + len {
            return Err(AdbError::Protocol(format!(
                "payload truncado: {} < {}",
                buf.len() - HEADER_LEN,
                len
            )));
        }
        let payload = buf[HEADER_LEN..HEADER_LEN + len].to_vec();
        if crc32(&payload) != crc {
            return Err(AdbError::Protocol("crc32 incorrecto".into()));
        }
        Ok(Message { command, arg0, arg1, payload })
    }

    /// Nombre legible del comando.
    pub fn command_name(&self) -> &'static str {
        match self.command {
            CMD_CNXN => "CNXN",
            CMD_AUTH => "AUTH",
            CMD_OPEN => "OPEN",
            CMD_OKAY => "OKAY",
            CMD_CLSE => "CLSE",
            CMD_WRTE => "WRTE",
            CMD_SYNC => "SYNC",
            _ => "?",
        }
    }
}

/// Cliente ADB sobre un socket TCP.
pub struct AdbClient {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl AdbClient {
    /// Conecta a `addr` (host:puerto) con timeout de conexión y de lectura.
    pub fn connect(addr: &str, timeout: Duration) -> Result<AdbClient> {
        let sa = addr
            .to_socket_addrs()
            .map_err(|e| AdbError::Io(e))?
            .next()
            .ok_or_else(|| AdbError::Protocol(format!("no se pudo resolver {addr}")))?;
        let stream = TcpStream::connect_timeout(&sa, timeout)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        stream.set_nodelay(true)?;
        Ok(AdbClient { stream, buf: Vec::with_capacity(HEADER_LEN + MAX_PAYLOAD as usize) })
    }

    /// Envía un mensaje.
    pub fn send(&mut self, m: &Message) -> Result<()> {
        self.stream.write_all(&m.encode())?;
        self.stream.flush()?;
        Ok(())
    }

    /// Recibe un mensaje completo (cabecera + payload).
    pub fn recv(&mut self) -> Result<Message> {
        self.buf.clear();
        self.buf.resize(HEADER_LEN, 0);
        self.stream.read_exact(&mut self.buf)?;
        let len = u32::from_le_bytes([self.buf[12], self.buf[13], self.buf[14], self.buf[15]]) as usize;
        if len > 0 {
            let mut payload = vec![0u8; len];
            self.stream.read_exact(&mut payload)?;
            self.buf.extend_from_slice(&payload);
        }
        Message::decode(&self.buf)
    }

    /// Cierra el socket de forma ordenada.
    pub fn close(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

/// Dispositivo listado por el servidor ADB.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Device {
    pub serial: String,
    pub state: String,
    pub product: Option<String>,
    pub model: Option<String>,
    pub device: Option<String>,
    pub transport_id: Option<u32>,
}

impl Device {
    /// ¿Está listo para aceptar comandos?
    pub fn is_ready(&self) -> bool {
        self.state == "device"
    }
}

/// Parsea la salida de `host:devices-l` (una línea por dispositivo).
pub fn parse_devices(out: &str) -> Vec<Device> {
    let mut v = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.split_whitespace();
        let serial = match it.next() {
            Some(s) => s.to_string(),
            None => continue,
        };
        let state = it.next().unwrap_or("unknown").to_string();
        let mut d = Device { serial, state, ..Default::default() };
        for kv in it {
            if let Some((k, v)) = kv.split_once(':') {
                match k {
                    "product" => d.product = Some(v.to_string()),
                    "model" => d.model = Some(v.to_string()),
                    "device" => d.device = Some(v.to_string()),
                    "transport_id" => d.transport_id = v.parse().ok(),
                    _ => {}
                }
            }
        }
        v.push(d);
    }
    v
}

/// Resultado de un comando shell.
#[derive(Debug, Clone, Default)]
pub struct ShellOutput {
    pub ok: bool,
    pub output: String,
    pub truncated: bool,
    pub elapsed_ms: u128,
}

/// Habla con un **servidor ADB** (`127.0.0.1:5037`).
pub struct AdbServer {
    addr: String,
    timeout: Duration,
}

impl AdbServer {
    pub fn new(addr: impl Into<String>, timeout: Duration) -> AdbServer {
        AdbServer { addr: addr.into(), timeout }
    }

    /// `host:devices-l`: lista los dispositivos conocidos por el servidor.
    pub fn devices(&self) -> Result<Vec<Device>> {
        let mut c = AdbClient::connect(&self.addr, self.timeout)?;
        let res = (|| -> Result<Vec<Device>> {
            c.send(&Message::new(CMD_CNXN, ADB_VERSION, MAX_PAYLOAD, Vec::new()))?;
            let mut body = Vec::new();
            loop {
                let m = c.recv()?;
                match m.command {
                    CMD_OKAY => {}
                    CMD_WRTE => {
                        if body.len() < DEFAULT_MAX_BYTES {
                            body.extend_from_slice(&m.payload);
                        }
                    }
                    CMD_CLSE => break,
                    CMD_CNXN => continue,
                    _ => {}
                }
                if m.command == CMD_CLSE {
                    break;
                }
            }
            Ok(parse_devices(&String::from_utf8_lossy(&body)))
        })();
        c.close();
        res
    }

    /// Ejecuta un comando en un dispositivo a través del servidor.
    pub fn shell(&self, serial: &str, cmd: &str, max_bytes: usize) -> Result<ShellOutput> {
        let t0 = Instant::now();
        let mut c = AdbClient::connect(&self.addr, self.timeout)?;
        let res = (|| -> Result<ShellOutput> {
            c.send(&Message::new(CMD_CNXN, ADB_VERSION, MAX_PAYLOAD, Vec::new()))?;
            let request = format!("host:transport:{serial}:shell:{cmd}");
            c.send(&Message::new(
                CMD_OPEN,
                0,
                0,
                request.into_bytes(),
            ))?;
            let mut out = Vec::new();
            let mut truncated = false;
            let mut ok = true;
            loop {
                let m = c.recv()?;
                match m.command {
                    CMD_OKAY => ok = ok || m.arg1 == 0,
                    CMD_WRTE => {
                        if out.len() + m.payload.len() > max_bytes {
                            let room = max_bytes.saturating_sub(out.len());
                            out.extend_from_slice(&m.payload[..room.min(m.payload.len())]);
                            truncated = true;
                            break;
                        }
                        out.extend_from_slice(&m.payload);
                    }
                    CMD_CLSE => break,
                    _ => {}
                }
            }
            Ok(ShellOutput {
                ok,
                truncated,
                output: String::from_utf8_lossy(&out).to_string(),
                elapsed_ms: t0.elapsed().as_millis(),
            })
        })();
        c.close();
        res
    }
}

/// Habla directamente con `adbd` (puerto TCP 5555): el único modo que
/// funciona desde el propio móvil, donde no hay servidor ADB.
pub struct Adbd {
    addr: String,
    timeout: Duration,
}

impl Adbd {
    pub fn new(host: &str, port: u16, timeout: Duration) -> Adbd {
        Adbd { addr: format!("{host}:{port}"), timeout }
    }

    /// Conecta y completa el handshake `CNXN`, **autenticando** si el
    /// dispositivo lo pide (`ro.adb.secure=1`).
    ///
    /// Si `adbd` manda `AUTH`, se firma el token con la clave de host y se
    /// responde `AUTH`.  Si el dispositivo rechaza la firma (vuelve a mandar
    /// `AUTH`), se reintenta el mismo handshake con la otra variante de hash
    /// del token (semántica C frente a bytes completos).
    ///
    /// Devuelve el cliente ya conectado y el banner del dispositivo.
    pub fn connect_authenticated(&self, key: Option<&HostKey>) -> Result<(AdbClient, String)> {
        let mut c = AdbClient::connect(&self.addr, self.timeout)?;
        let mut banner = HOST_BANNER.as_bytes().to_vec();
        banner.push(0);
        c.send(&Message::new(CMD_CNXN, ADB_VERSION, MAX_PAYLOAD, banner))?;
        let mut m = c.recv()?;
        if m.command == CMD_AUTH {
            let key = key.ok_or_else(|| {
                AdbError::Protocol(
                    "adbd pide autenticacion (AUTH) y no hay clave de host cargada: ponla en \
                     <state_dir>/adb_host_key.pem (0600) y registra su parte publica en \
                     /data/misc/adb/adb_keys"
                        .into(),
                )
            })?;
            for mode in [TokenHash::CStr, TokenHash::Full] {
                let sig = key.sign_token(&m.payload, mode)?;
                c.send(&Message::new(CMD_AUTH, m.arg0, 0, sig))?;
                m = c.recv()?;
                match m.command {
                    CMD_CNXN => {
                        let dev_banner = String::from_utf8_lossy(&m.payload)
                            .trim_end_matches('\0')
                            .to_string();
                        return Ok((c, dev_banner));
                    }
                    CMD_AUTH => continue,
                    other => {
                        return Err(AdbError::Protocol(format!(
                            "respuesta inesperada a AUTH: {}",
                            Message::new(other, 0, 0, Vec::new()).command_name()
                        )));
                    }
                }
            }
            return Err(AdbError::Protocol(
                "adbd rechazo la firma de la clave de host (public key no registrada o clave distinta)".into(),
            ));
        }
        if m.command != CMD_CNXN {
            return Err(AdbError::Protocol(format!(
                "respuesta inesperada a CNXN: {}",
                m.command_name()
            )));
        }
        let dev_banner = String::from_utf8_lossy(&m.payload)
            .trim_end_matches('\0')
            .to_string();
        Ok((c, dev_banner))
    }

    /// Handshake `CNXN`: devuelve el banner del dispositivo.
    pub fn banner(&self) -> Result<String> {
        let key = host_key();
        let (mut c, banner) = self.connect_authenticated(key.as_ref())?;
        c.close();
        Ok(banner)
    }

    /// Ejecuta `cmd` en el dispositivo (`shell:<cmd>`) y devuelve la salida.
    pub fn shell(&self, cmd: &str, max_bytes: usize) -> Result<ShellOutput> {
        let t0 = Instant::now();
        let key = host_key();
        let (mut c, _banner) = self.connect_authenticated(key.as_ref())?;
        let res = (|| -> Result<ShellOutput> {
            let stream_id = 1u32;
            c.send(&Message::new(
                CMD_OPEN,
                stream_id,
                0,
                format!("shell:{cmd}").into_bytes(),
            ))?;
            // adbd responde OKAY con arg0 = stream_id; a partir de ahí llegan
            // paquetes WRTE con la salida y un CLSE al final.
            let mut out: Vec<u8> = Vec::new();
            let mut truncated = false;
            let mut opened = false;
            loop {
                let m = c.recv()?;
                match m.command {
                    CMD_OKAY => {
                        if m.arg0 == stream_id {
                            opened = true;
                        }
                    }
                    CMD_WRTE => {
                        if out.len() + m.payload.len() > max_bytes {
                            let room = max_bytes.saturating_sub(out.len());
                            out.extend_from_slice(&m.payload[..room.min(m.payload.len())]);
                            truncated = true;
                            break;
                        }
                        out.extend_from_slice(&m.payload);
                    }
                    CMD_CLSE => break,
                    _ => {}
                }
            }
            if !opened {
                return Err(AdbError::Protocol(
                    "adbd no abrio el stream (OPEN sin OKAY)".into(),
                ));
            }
            Ok(ShellOutput {
                ok: true,
                truncated,
                output: String::from_utf8_lossy(&out).to_string(),
                elapsed_ms: t0.elapsed().as_millis(),
            })
        })();
        c.close();
        res
    }

    /// Sube un fichero al dispositivo (servicio `sync:` de ADB).
    ///
    /// `dst` es la ruta **en el dispositivo**.  El protocolo es el SYNC v1 de
    /// AOSP: `SEND <ruta>` → bloques `DATA` → `DONE`, y el dispositivo
    /// responde `OKAY` o `FAIL <len> <mensaje>`.
    pub fn push_file(&self, dst: &str, data: &[u8]) -> Result<usize> {
        let key = host_key();
        let (mut c, _) = self.connect_authenticated(key.as_ref())?;
        let stream_id = 1u32;
        c.send(&Message::new(
            CMD_OPEN,
            stream_id,
            0,
            b"sync:".to_vec(),
        ))?;
        // El daemon responde OKAY al OPEN.
        let mut opened = false;
        loop {
            let m = c.recv()?;
            if m.command == CMD_OKAY && m.arg0 == stream_id {
                opened = true;
                break;
            }
            if m.command == CMD_CLSE {
                break;
            }
        }
        if !opened {
            c.close();
            return Err(AdbError::Protocol(
                "adbd no abrio el stream sync (push)".into(),
            ));
        }

        let res = (|| -> Result<usize> {
            // SEND <ruta>\0
            let mut req = Vec::with_capacity(dst.len() + 5);
            req.extend_from_slice(&SYNC_SEND.to_le_bytes());
            req.extend_from_slice(dst.as_bytes());
            req.push(0);
            c.send(&Message::new(CMD_WRTE, stream_id, 0, req))?;

            // Bloques DATA
            let mut sent = 0usize;
            for chunk in data.chunks(SYNC_CHUNK) {
                let mut blk = Vec::with_capacity(8 + chunk.len());
                blk.extend_from_slice(&SYNC_DATA.to_le_bytes());
                blk.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
                blk.extend_from_slice(chunk);
                c.send(&Message::new(CMD_WRTE, stream_id, 0, blk))?;
                sent += chunk.len();
            }

            // DONE + mtime (0 = ahora)
            let mut done = Vec::with_capacity(8);
            done.extend_from_slice(&SYNC_DONE.to_le_bytes());
            done.extend_from_slice(&0u32.to_le_bytes());
            c.send(&Message::new(CMD_WRTE, stream_id, 0, done))?;

            // Respuesta final: OKAY o FAIL.
            let (id, payload) = read_sync_packet(&mut c, stream_id)?;
            match id {
                SYNC_OKAY => Ok(sent),
                SYNC_FAIL => Err(AdbError::Protocol(format!(
                    "adbd rechazo el push: {}",
                    String::from_utf8_lossy(&payload)
                ))),
                other => Err(AdbError::Protocol(format!(
                    "respuesta inesperada al push: id={:#x}",
                    other
                ))),
            }
        })();
        c.close();
        res
    }

    /// Baja un fichero del dispositivo (servicio `sync:` de ADB).
    ///
    /// `src` es la ruta **en el dispositivo**.  El tamaño devuelto está
    /// limitado por `max_bytes`: si el fichero es mayor, se trunca.
    pub fn pull_file(&self, src: &str, max_bytes: usize) -> Result<Vec<u8>> {
        let key = host_key();
        let (mut c, _) = self.connect_authenticated(key.as_ref())?;
        let stream_id = 1u32;
        c.send(&Message::new(
            CMD_OPEN,
            stream_id,
            0,
            b"sync:".to_vec(),
        ))?;
        let mut opened = false;
        loop {
            let m = c.recv()?;
            if m.command == CMD_OKAY && m.arg0 == stream_id {
                opened = true;
                break;
            }
            if m.command == CMD_CLSE {
                break;
            }
        }
        if !opened {
            c.close();
            return Err(AdbError::Protocol(
                "adbd no abrio el stream sync (pull)".into(),
            ));
        }

        let res = (|| -> Result<Vec<u8>> {
            let mut req = Vec::with_capacity(src.len() + 5);
            req.extend_from_slice(&SYNC_RECV.to_le_bytes());
            req.extend_from_slice(src.as_bytes());
            req.push(0);
            c.send(&Message::new(CMD_WRTE, stream_id, 0, req))?;

            let mut out: Vec<u8> = Vec::new();
            let mut truncated = false;
            loop {
                let (id, payload) = read_sync_packet(&mut c, stream_id)?;
                match id {
                    SYNC_DATA => {
                        if out.len() + payload.len() > max_bytes {
                            let room = max_bytes.saturating_sub(out.len());
                            out.extend_from_slice(&payload[..room.min(payload.len())]);
                            truncated = true;
                            break;
                        }
                        out.extend_from_slice(&payload);
                    }
                    SYNC_DONE => break,
                    SYNC_FAIL => {
                        return Err(AdbError::Protocol(format!(
                            "adbd no pudo leer {src}: {}",
                            String::from_utf8_lossy(&payload)
                        )))
                    }
                    SYNC_OKAY => continue,
                    other => {
                        return Err(AdbError::Protocol(format!(
                            "respuesta inesperada al pull: id={:#x}",
                            other
                        )))
                    }
                }
            }
            let _ = truncated;
            Ok(out)
        })();
        c.close();
        res
    }
}

/// Lee el siguiente paquete del protocolo SYNC (4 bytes de id + payload),
/// acumulando los mensajes `WRTE` del transporte ADB.
fn read_sync_packet(c: &mut AdbClient, stream_id: u32) -> Result<(u32, Vec<u8>)> {
    let mut buf: Vec<u8> = Vec::new();
    loop {
        if buf.len() >= 4 {
            let id = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
            match id {
                SYNC_DATA => {
                    if buf.len() >= 8 {
                        let len =
                            u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
                        if buf.len() >= 8 + len {
                            let payload = buf[8..8 + len].to_vec();
                            buf.drain(..8 + len);
                            return Ok((id, payload));
                        }
                    }
                }
                SYNC_DONE => {
                    if buf.len() >= 8 {
                        let payload = buf[4..8].to_vec();
                        buf.drain(..8);
                        return Ok((id, payload));
                    }
                }
                SYNC_OKAY => {
                    if buf.len() >= 4 {
                        buf.drain(..4);
                        return Ok((id, Vec::new()));
                    }
                }
                SYNC_FAIL => {
                    // FAIL <len u32> <mensaje>
                    if buf.len() >= 8 {
                        let len =
                            u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
                        if buf.len() >= 8 + len {
                            let payload = buf[8..8 + len].to_vec();
                            buf.drain(..8 + len);
                            return Ok((id, payload));
                        }
                    }
                }
                _ => {
                    // Id desconocido: se descarta el resto del buffer.
                    buf.clear();
                    return Err(AdbError::Protocol(format!(
                        "paquete SYNC desconocido: id={:#x}",
                        id
                    )));
                }
            }
        }
        let m = c.recv()?;
        match m.command {
            CMD_WRTE => buf.extend_from_slice(&m.payload),
            CMD_OKAY => continue,
            CMD_CLSE => {
                return Err(AdbError::Protocol(format!(
                    "adbd cerro el stream sync (stream {stream_id})"
                )))
            }
            _ => continue,
        }
    }
}

/// Estado resumido de ADB en el dispositivo (para /api/adb/status y doctor).
#[derive(Debug, Clone, Default)]
pub struct AdbStatus {
    /// ¿Hay un servidor ADB local (127.0.0.1:5037)?
    pub server_ok: bool,
    /// Dispositivos vistos por el servidor local.
    pub devices: Vec<Device>,
    /// ¿Responde `adbd` por TCP?
    pub adbd_ok: bool,
    /// Banner de `adbd` (modelo, versión...).
    pub adbd_banner: String,
    /// Último error (texto, sin secretos).
    pub error: Option<String>,
}

/// Sonda completa: servidor ADB local + adbd por TCP.
pub fn status(server_addr: &str, adbd_host: &str, adbd_port: u16, timeout: Duration) -> AdbStatus {
    let mut st = AdbStatus::default();
    match AdbServer::new(server_addr, timeout).devices() {
        Ok(d) => {
            st.server_ok = true;
            st.devices = d;
        }
        Err(e) => st.error = Some(e.to_string()),
    }
    match Adbd::new(adbd_host, adbd_port, timeout).banner() {
        Ok(b) => {
            st.adbd_ok = true;
            st.adbd_banner = b;
        }
        Err(e) => {
            if st.error.is_none() {
                st.error = Some(e.to_string());
            }
        }
    }
    st
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    // ---- unidad ---------------------------------------------------------

    #[test]
    fn crc32_coincide_con_zlib() {
        // valores de referencia de zlib.crc32
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"The quick brown fox jumps over the lazy dog"), 0x414F_A339);
    }

    #[test]
    fn mensaje_ida_y_vuelta() {
        let m = Message::new(CMD_CNXN, ADB_VERSION, MAX_PAYLOAD, b"host::features=shell_v2\0".to_vec());
        let bytes = m.encode();
        assert_eq!(bytes.len(), HEADER_LEN + m.payload.len());
        let back = Message::decode(&bytes).expect("decode");
        assert_eq!(back, m);
        assert_eq!(back.command_name(), "CNXN");
    }

    #[test]
    fn mensaje_con_crc_corrupto_falla() {
        let mut bytes = Message::new(CMD_WRTE, 1, 0, b"hola".to_vec()).encode();
        let n = bytes.len();
        bytes[n - 1] ^= 0xFF; // tocca el payload -> crc mismatch
        assert!(Message::decode(&bytes).is_err());
    }

    #[test]
    fn magic_incorrecto_falla() {
        let mut bytes = Message::new(CMD_OKAY, 1, 0, Vec::new()).encode();
        bytes[20] ^= 0xFF;
        assert!(Message::decode(&bytes).is_err());
    }

    #[test]
    fn parse_devices_lee_hoja_l() {
        let out = "emulator-5554\tdevice product:sdk model:Pixel device:generic transport_id:1\n\
                   R58M12345\toffline\n";
        let d = parse_devices(out);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].serial, "emulator-5554");
        assert_eq!(d[0].state, "device");
        assert_eq!(d[0].model.as_deref(), Some("Pixel"));
        assert_eq!(d[0].transport_id, Some(1));
        assert!(d[0].is_ready());
        assert_eq!(d[1].state, "offline");
        assert!(!d[1].is_ready());
    }

    // ---- integración: adbd falso ----------------------------------------

    /// adbd simulado: responde al CNXN y sirve `shell:` con una salida fija.
    fn fake_adbd(script: &'static str) -> (String, thread::JoinHandle<()>) {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = l.local_addr().expect("addr").to_string();
        let (tx, rx) = mpsc::channel::<String>();
        let h = thread::spawn(move || {
            for stream in l.incoming() {
                let mut s = stream.expect("accept");
                let _ = tx.send(s.peer_addr().map(|a| a.to_string()).unwrap_or_default());
                loop {
                    let m = match read_message(&mut s) {
                        Some(m) => m,
                        None => break,
                    };
                    match m.command {
                        CMD_CNXN => {
                            let banner = b"device::ro.product.name=samsung_x;ro.product.model=SM-A536E\0".to_vec();
                            s.write_all(&Message::new(CMD_CNXN, 0x0100_0000, MAX_PAYLOAD, banner).encode())
                                .ok();
                        }
                        CMD_OPEN => {
                            let req = String::from_utf8_lossy(&m.payload).to_string();
                            s.write_all(&Message::new(CMD_OKAY, m.arg0, 0, Vec::new()).encode()).ok();
                            if req.starts_with("shell:") {
                                let out = script.as_bytes().to_vec();
                                s.write_all(&Message::new(CMD_WRTE, m.arg0, 0, out).encode()).ok();
                            }
                            s.write_all(&Message::new(CMD_CLSE, m.arg0, 0, Vec::new()).encode()).ok();
                        }
                        _ => {}
                    }
                }
            }
        });
        (addr, h)
    }

    fn read_message(s: &mut TcpStream) -> Option<Message> {
        let mut hdr = [0u8; HEADER_LEN];
        s.read_exact(&mut hdr).ok()?;
        let len = u32::from_le_bytes([hdr[12], hdr[13], hdr[14], hdr[15]]) as usize;
        let mut payload = vec![0u8; len];
        if len > 0 {
            s.read_exact(&mut payload).ok()?;
        }
        let mut all = hdr.to_vec();
        all.extend_from_slice(&payload);
        Message::decode(&all).ok()
    }

    #[test]
    fn adbd_directo_devuelve_banner_y_salida() {
        let (addr, _h) = fake_adbd("hola desde adbd\n");
        let (host, port) = addr.rsplit_once(':').expect("addr");
        let port: u16 = port.parse().expect("port");
        let adbd = Adbd::new(host, port, Duration::from_secs(5));

        let banner = adbd.banner().expect("banner");
        assert!(banner.contains("ro.product.model=SM-A536E"), "banner: {banner}");

        let out = adbd.shell("id", DEFAULT_MAX_BYTES).expect("shell");
        assert!(out.ok);
        assert_eq!(out.output, "hola desde adbd\n");
        assert!(!out.truncated);
    }

    #[test]
    fn adbd_directo_recorta_a_max_bytes() {
        let (addr, _h) = fake_adbd("0123456789ABCDEF");
        let (host, port) = addr.rsplit_once(':').expect("addr");
        let port: u16 = port.parse().expect("port");
        let out = Adbd::new(host, port, Duration::from_secs(5))
            .shell("id", 4)
            .expect("shell");
        assert_eq!(out.output.len(), 4);
        assert!(out.truncated);
    }

    #[test]
    fn adbd_inalcanzable_da_error_claro() {
        // puerto cerrado: ni listener ni hairpin -> error de io, no cuelgue
        let err = Adbd::new("127.0.0.1", 1, Duration::from_millis(300))
            .banner()
            .expect_err("debe fallar");
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn servidor_adb_falso_lista_dispositivos() {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = l.local_addr().expect("addr").to_string();
        let h = thread::spawn(move || {
            for stream in l.incoming() {
                let mut s = stream.expect("accept");
                loop {
                    let m = match read_message(&mut s) {
                        Some(m) => m,
                        None => break,
                    };
                    if m.command == CMD_CNXN {
                        s.write_all(&Message::new(CMD_OKAY, 1, 0, Vec::new()).encode()).ok();
                        let body = b"192.168.1.124:5555\tdevice product:samsung model:SM-A536E device:a53 transport_id:3\n";
                        s.write_all(&Message::new(CMD_WRTE, 1, 0, body.to_vec()).encode()).ok();
                        s.write_all(&Message::new(CMD_CLSE, 1, 0, Vec::new()).encode()).ok();
                    }
                }
            }
        });
        let devs = AdbServer::new(addr, Duration::from_secs(5)).devices().expect("devices");
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].serial, "192.168.1.124:5555");
        assert_eq!(devs[0].model.as_deref(), Some("SM-A536E"));
        assert!(devs[0].is_ready());
        drop(h);
    }
}
