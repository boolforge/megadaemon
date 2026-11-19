//! MegaDaemon v13.4.1 — Tool ADB nativa.
//!
//! Expone el cliente ADB en Rust puro (`crate::adb_client`) como herramienta
//! MCP `adb`, de modo que toda la flota pueda hablar con el dispositivo sin
//! el binario `adb` y **a través del túnel**: la ruta de servicio no depende
//! de ninguna IP de la LAN y, por tanto, no la afecta un bloqueo por IP.
//!
//! Acciones:
//!   * `status`  — ¿hay `adbd`? ¿banner? ¿clave de host cargada?
//!   * `shell`   — ejecuta una orden en el dispositivo (`shell:<cmd>`).
//!   * `devices` — dispositivos vistos por el servidor ADB local (5037).
//!   * `keyinfo` — estado de la clave de host (nunca la clave en sí).
//!   * `push`    — sube un fichero al dispositivo (protocolo SYNC).
//!   * `pull`    — baja un fichero del dispositivo (protocolo SYNC).
//!
//! La clave de host **nunca** sale del disco: sólo se firma el token `AUTH`
//! en memoria y el estado que se publica no contiene material criptográfico.

use serde_json::{json, Value};
use std::time::Duration;

use crate::adb_client::{self, AdbError, Adbd};

/// Tope por defecto de salida de un comando (256 KiB).
const DEFAULT_MAX_BYTES: usize = 256 * 1024;
/// Timeout por defecto de conexión (10 s).
const DEFAULT_TIMEOUT_SECS: u64 = 10;

/// Catálogo de la herramienta.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "adb",
            "description": "ADB nativo en Rust contra el dispositivo (sin binario `adb`): estado de adbd, shell, push/pull de ficheros y clave de host. La ruta va por el tunel: inmune al bloqueo por IP.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["status", "shell", "devices", "keyinfo", "push", "pull"],
                        "default": "status"
                    },
                    "command": { "type": "string", "description": "Orden a ejecutar en el dispositivo (action=shell)." },
                    "host": { "type": "string", "default": "127.0.0.1", "description": "Host de adbd. 127.0.0.1 desde el propio movil; 192.168.1.124:5555 desde la LAN." },
                    "port": { "type": "integer", "default": 5555, "description": "Puerto TCP de adbd." },
                    "src": { "type": "string", "description": "Fichero local (action=push) o ruta remota (action=pull)." },
                    "dst": { "type": "string", "description": "Ruta remota (action=push) o fichero local (action=pull)." },
                    "max_bytes": { "type": "integer", "default": 262144, "description": "Tope de salida en bytes." },
                    "timeout_secs": { "type": "integer", "default": 10 }
                },
                "required": ["action"]
            }
        }
    ])
}

/// Punto de entrada de la herramienta.  `None` si el nombre no es el suyo.
pub fn dispatch(name: &str, args: &Value) -> Option<Value> {
    if name != "adb" {
        return None;
    }
    Some(run(args))
}

fn run(args: &Value) -> Value {
    let action = args["action"].as_str().unwrap_or("status");
    let host = args["host"].as_str().unwrap_or("127.0.0.1");
    let port = args["port"].as_u64().unwrap_or(5555) as u16;
    let timeout = Duration::from_secs(args["timeout_secs"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS));
    let max_bytes = args["max_bytes"].as_u64().unwrap_or(DEFAULT_MAX_BYTES as u64) as usize;

    let res = match action {
        "status" => status(host, port, timeout),
        "shell" => shell(host, port, timeout, args, max_bytes),
        "devices" => devices(timeout),
        "keyinfo" => keyinfo(),
        "push" => push(host, port, timeout, args),
        "pull" => pull(host, port, timeout, args, max_bytes),
        other => Err(format!("accion desconocida: {other}")),
    };

    match res {
        Ok(v) => {
            crate::android::audit("adb", &json!({ "action": action, "host": host, "port": port, "ok": true }));
            v
        }
        Err(e) => {
            crate::android::audit("adb", &json!({ "action": action, "host": host, "port": port, "ok": false, "error": e }));
            json!({ "ok": false, "error": e })
        }
    }
}

/// `status`: ¿responde `adbd`? ¿qué banner? ¿hay clave de host?
fn status(host: &str, port: u16, timeout: Duration) -> Result<Value, String> {
    let adbd = Adbd::new(host, port, timeout);
    let key = adb_client::host_key();
    let key_path = adb_client::active_host_key_path()
        .map(|p| p.display().to_string());
    let (adbd_ok, banner, err) = match adbd.banner() {
        Ok(b) => (true, b, None),
        Err(e) => (false, String::new(), Some(e.to_string())),
    };
    let server_ok = adb_client::AdbServer::new("127.0.0.1:5037", timeout).devices().is_ok();
    Ok(json!({
        "ok": adbd_ok,
        "adbd_ok": adbd_ok,
        "adbd_host": host,
        "adbd_port": port,
        "banner": banner,
        "error": err,
        "adb_server_local": server_ok,
        "host_key_present": key.is_some(),
        "host_key_path": key_path,
        "host_key_bits": key.as_ref().map(|k| k.bits()),
        "auth": "RSA-SHA1 (PKCS#1 v1.5) sobre el token AUTH de 20 bytes"
    }))
}

/// `shell`: ejecuta una orden en el dispositivo.
fn shell(host: &str, port: u16, timeout: Duration, args: &Value, max_bytes: usize) -> Result<Value, String> {
    let cmd = args["command"].as_str().unwrap_or("").trim();
    if cmd.is_empty() {
        return Err("falta 'command' para action=shell".into());
    }
    let adbd = Adbd::new(host, port, timeout);
    let out = adbd.shell(cmd, max_bytes).map_err(|e| e.to_string())?;
    Ok(json!({
        "ok": out.ok,
        "command": cmd,
        "output": out.output,
        "truncated": out.truncated,
        "elapsed_ms": out.elapsed_ms
    }))
}

/// `devices`: dispositivos del servidor ADB local (5037).
fn devices(timeout: Duration) -> Result<Value, String> {
    let devs = adb_client::AdbServer::new("127.0.0.1:5037", timeout)
        .devices()
        .map_err(|e| e.to_string())?;
    Ok(json!({
        "ok": true,
        "count": devs.len(),
        "devices": devs.iter().map(|d| json!({
            "serial": d.serial,
            "state": d.state,
            "ready": d.is_ready()
        })).collect::<Vec<_>>()
    }))
}

/// `keyinfo`: estado de la clave de host (sin material criptográfico).
fn keyinfo() -> Result<Value, String> {
    let key = adb_client::host_key();
    let path = adb_client::active_host_key_path().map(|p| p.display().to_string());
    let candidates: Vec<String> = adb_client::host_key_candidates()
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    Ok(json!({
        "ok": true,
        "present": key.is_some(),
        "path": path,
        "bits": key.as_ref().map(|k| k.bits()),
        "candidates": candidates,
        "note": "La clave privada no sale del disco. Registra su parte publica en /data/misc/adb/adb_keys como 'usuario@host <base64 SPKI>'."
    }))
}

/// `push`: sube un fichero al dispositivo (protocolo SYNC de ADB).
fn push(host: &str, port: u16, timeout: Duration, args: &Value) -> Result<Value, String> {
    let src = args["src"].as_str().unwrap_or("");
    let dst = args["dst"].as_str().unwrap_or("");
    if src.is_empty() || dst.is_empty() {
        return Err("push necesita 'src' y 'dst'".into());
    }
    let data = std::fs::read(src).map_err(|e| format!("no se pudo leer {src}: {e}"))?;
    let n = adb_client::Adbd::new(host, port, timeout)
        .push_file(dst, &data)
        .map_err(|e| e.to_string())?;
    Ok(json!({ "ok": true, "src": src, "dst": dst, "bytes": n }))
}

/// `pull`: baja un fichero del dispositivo (protocolo SYNC de ADB).
fn pull(host: &str, port: u16, timeout: Duration, args: &Value, max_bytes: usize) -> Result<Value, String> {
    let src = args["src"].as_str().unwrap_or("");
    let dst = args["dst"].as_str().unwrap_or("");
    if src.is_empty() {
        return Err("pull necesita 'src'".into());
    }
    let data = adb_client::Adbd::new(host, port, timeout)
        .pull_file(src, max_bytes)
        .map_err(|e| e.to_string())?;
    if !dst.is_empty() {
        std::fs::write(dst, &data).map_err(|e| format!("no se pudo escribir {dst}: {e}"))?;
        Ok(json!({ "ok": true, "src": src, "dst": dst, "bytes": data.len() }))
    } else {
        // Sin destino: se devuelve el contenido en base64 (coteado).
        let b64 = base64ct::Base64::encode_string(&data);
        Ok(json!({ "ok": true, "src": src, "bytes": data.len(), "base64": b64 }))
    }
}

/// Traduce un error del cliente ADB a texto plano.
pub fn describe_error(e: &AdbError) -> String {
    e.to_string()
}
