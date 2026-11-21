//! W2 · Driver del inventario del dispositivo.
//!
//! Ejecuta el plan de captura de `device_inventory` contra el móvil real (por
//! MCP `root_exec`), se trae cada fichero de referencia **por trozos** (el
//! móvil va al 98 % de disco y con la RAM justa) y escribe:
//!
//! ```text
//! <salida>/inventory.json        inventario estructurado y versionado
//! <salida>/MANIFEST.ndjson       una línea por fichero de referencia con su sha256
//! <salida>/refs/<id>             los ficheros de referencia tal cual
//! ```
//!
//! Uso:
//! ```bash
//! MEGADAEMON_URL=... MEGADAEMON_API_KEY=... MEGADAEMON_COCKPIT_TOKEN=... \
//!   cargo run --release --bin md-device-inventory -- --out /workspace/inv
//! ```
//!
//! Las credenciales se leen del entorno o de `--secrets <fichero>` (formato
//! `CLAVE=valor`), que NUNCA se escribe en el repo.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[path = "../device_inventory.rs"]
mod device_inventory;

use device_inventory::{
    capture_plan, copy_plan, manifest_ndjson, sha256_hex, FileDigest, Options, Source,
};

/// Tamaño del trozo (en bytes crudos) que se pide al móvil cada vez.
const CHUNK: u64 = 256 * 1024;

/// Cliente MCP mínimo (JSON-RPC 2.0 sobre HTTP) — sin dependencias extra.
struct Mcp {
    url: String,
    key: String,
    token: String,
    id: u64,
}

impl Mcp {
    fn from_env(secrets: Option<&str>) -> Result<Self, String> {
        let mut kv: Vec<(String, String)> = Vec::new();
        if let Some(path) = secrets {
            let txt = fs::read_to_string(path).map_err(|e| format!("no se pudo leer {path}: {e}"))?;
            for line in txt.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    kv.push((k.trim().to_string(), v.trim().trim_matches('"').to_string()));
                }
            }
        }
        // PRECEDENCIA: el fichero de secretos manda sobre el entorno. Una key
        // obsoleta exportada en la shell hace que el cliente mande una key
        // revocada → 403 → "el móvil está caído" cuando está perfecto.
        let get = |name: &str| -> String {
            if let Some((_, v)) = kv.iter().find(|(k, _)| k == name) {
                if !v.is_empty() {
                    return v.clone();
                }
            }
            std::env::var(name).unwrap_or_default()
        };
        // El endpoint MCP NO es la raíz del cockpit: es `<url>/mcp`. Si se
        // postea a la raíz, el daemon responde otra cosa y parece un fallo
        // del móvil cuando lo que falla es la ruta.
        let mut url = get("MEGADAEMON_MCP");
        if url.is_empty() {
            url = get("MEGADAEMON_URL");
        }
        if url.is_empty() {
            return Err("falta MEGADAEMON_MCP (o MEGADAEMON_URL)".to_string());
        }
        if !url.contains("/mcp") {
            url = format!("{}/mcp", url.trim_end_matches('/'));
        }
        let key = get("MEGADAEMON_API_KEY");
        let token = get("MEGADAEMON_COCKPIT_TOKEN");
        // Diagnóstico sin imprimir secretos: sólo el sha16 de cada valor.
        eprintln!(
            "[inv] credenciales: key sha16 {} · token sha16 {}",
            &sha256_hex(key.as_bytes())[..16],
            &sha256_hex(token.as_bytes())[..16]
        );
        Ok(Mcp {
            url: url.trim_end_matches('/').to_string(),
            key,
            token,
            id: 1,
        })
    }

    /// Llamada JSON-RPC; devuelve el JSON de `result`.
    fn rpc(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.id += 1;
        let body = json!({
            "jsonrpc": "2.0",
            "id": self.id,
            "method": method,
            "params": params,
        })
        .to_string();
        let mut req = ureq::post(&self.url)
            .header("Content-Type", "application/json")
            .header("X-API-Key", &self.key);
        if !self.token.is_empty() {
            req = req.header("X-Seere-Auth-Token", &self.token);
        }
        let mut resp = req
            .send(body)
            .map_err(|e| format!("fallo HTTP: {e}"))?;
        let txt = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("respuesta no UTF-8: {e}"))?;
        let v: Value = serde_json::from_str(&txt).map_err(|e| format!("JSON inválido: {e}"))?;
        if let Some(err) = v.get("error") {
            return Err(format!("error RPC: {err}"));
        }
        let result = v.get("result").cloned().unwrap_or(Value::Null);
        // El daemon responde MCP "estándar": el payload de la tool va en
        // `structuredContent` (o en `content[0].text` como JSON en texto).
        if let Some(sc) = result.get("structuredContent") {
            if sc.is_object() {
                return Ok(sc.clone());
            }
        }
        if let Some(text) = result
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
        {
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                return Ok(parsed);
            }
        }
        Ok(result)
    }

    /// Handshake MCP: el daemon espera `initialize` antes de `tools/call`.
    fn initialize(&mut self) -> Result<(), String> {
        self.rpc(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "md-device-inventory", "version": "1"},
            }),
        )
        .map(|_| ())
    }

    /// Ejecuta un comando como root en el móvil y devuelve su salida.
    fn root_exec(&mut self, cmd: &str) -> Result<String, String> {
        self.root_exec_with(cmd, 180_000, 512 * 1024)
    }

    /// Igual, pero con timeout y tope de salida explícitos.
    fn root_exec_with(
        &mut self,
        cmd: &str,
        timeout_ms: u64,
        max_chars: usize,
    ) -> Result<String, String> {
        let res = self.rpc(
            "tools/call",
            json!({"name": "root_exec", "arguments": {
                "command": cmd,
                "timeout_ms": timeout_ms,
                "max_chars": max_chars
            }}),
        )?;
        // El daemon anida el resultado de la tool en "result" (o lo deja en
        // la raíz según la ruta): se aceptan las dos formas.
        let inner = res.get("result").unwrap_or(&res);
        let stdout = inner
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let ok = res
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| inner.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
        if !ok {
            let code = inner
                .get("exit_code")
                .and_then(|v| v.as_i64())
                .unwrap_or(-1);
            let err = inner.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
            return Err(format!("root_exec ok=false (exit {code}): {err}"));
        }
        Ok(stdout)
    }
}

/// Fuente real: los ficheros de referencia ya capturados en el móvil.
struct DeviceSource {
    files: Vec<(String, String)>,
    digests: Vec<(String, FileDigest)>,
}

impl Source for DeviceSource {
    fn shell(&self, _cmd: &str) -> Option<String> {
        None
    }
    fn file(&self, name: &str) -> Option<String> {
        self.files.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    }
    fn digest(&self, name: &str) -> Option<FileDigest> {
        self.digests.iter().find(|(k, _)| k == name).map(|(_, d)| d.clone())
    }
}

/// Base64 estándar (el móvil usa `base64 -w 0`).
fn decode_b64(s: &str) -> Vec<u8> {
    const TAB: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut rev = [255u8; 256];
    for (i, c) in TAB.iter().enumerate() {
        rev[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for c in s.bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = rev[c as usize];
        if v == 255 {
            continue;
        }
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

/// Genera el comando de captura: escribe cada referencia en un fichero y
/// calcula su SHA-256 en el propio móvil.
fn build_capture_cmd(dir: &str, batch: &[&str]) -> String {
    let mut cmd = format!("D={dir}; mkdir -p $D; ");
    for id in batch {
        if let Some(c) = capture_plan().iter().find(|c| c.id == *id) {
            cmd.push_str(&format!("({}) > $D/{} 2>/dev/null || true; ", c.cmd, c.id));
        }
        if let Some(c) = copy_plan().iter().find(|c| c.id == *id) {
            cmd.push_str(&format!("cp {} $D/{} 2>/dev/null || true; ", c.path, c.id));
        }
    }
    cmd
}

/// Reensambla el inventario desde un directorio de referencias ya capturado.
/// No contacta con el móvil: sólo lee ficheros locales y recalcula digests.
fn assemble_from_refs(dir: &str, out_dir: &str) -> Result<(), String> {
    let refs_dir = format!("{dir}/refs");
    let mut files: Vec<(String, String)> = Vec::new();
    let mut digests: Vec<(String, FileDigest)> = Vec::new();
    for id in device_inventory::capture_plan().iter().map(|c| c.id).chain(
        device_inventory::copy_plan().iter().map(|c| c.id),
    ) {
        let path = format!("{refs_dir}/{id}");
        let content = match fs::read(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let sha = sha256_hex(&content);
        digests.push((
            id.to_string(),
            FileDigest {
                sha256: sha,
                bytes: content.len() as u64,
            },
        ));
        files.push((id.to_string(), String::from_utf8_lossy(&content).to_string()));
    }
    let src = DeviceSource { files, digests };
    let inv = device_inventory::collect(&src, &Options::default());
    let manifest = manifest_ndjson(&src);
    fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {out_dir}: {e}"))?;
    let inv_path = format!("{out_dir}/inventory.json");
    let man_path = format!("{out_dir}/MANIFEST.ndjson");
    fs::write(
        &inv_path,
        serde_json::to_string_pretty(&inv).map_err(|e| format!("serializar: {e}"))?,
    )
    .map_err(|e| format!("escribir {inv_path}: {e}"))?;
    fs::write(&man_path, manifest.as_bytes())
        .map_err(|e| format!("escribir {man_path}: {e}"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "inventory": inv_path,
            "manifest": man_path,
            "referencias": inv["referencias"].as_array().map(|v| v.len()).unwrap_or(0),
            "apps_total": inv["apps"]["total"],
            "sha256_inventory": sha256_hex(fs::read(&inv_path).unwrap_or_default().as_slice()),
        }))
        .map_err(|e| format!("serializar resumen: {e}"))?
    );
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("md-device-inventory: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    // --- argumentos ---
    let args: Vec<String> = std::env::args().collect();
    let mut out_dir = "/workspace/inventario".to_string();
    let mut secrets: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_dir = args.get(i).cloned().unwrap_or(out_dir);
            }
            "--secrets" => {
                i += 1;
                secrets = args.get(i).cloned();
            }
            _ => {}
        }
        i += 1;
    }

    // Modo "--from-refs <dir>": reensambla el inventario desde los ficheros de
    // referencia YA capturados, SIN volver a tocar el móvil (el bus puede
    // estar instalando el binario y el móvil va justo de RAM).
    let mut from_refs: Option<String> = None;
    {
        let mut i = 1;
        while i < args.len() {
            if args[i] == "--from-refs" {
                i += 1;
                from_refs = args.get(i).cloned();
            }
            i += 1;
        }
    }
    if let Some(dir) = from_refs {
        return assemble_from_refs(&dir, &out_dir);
    }

    let mut mcp = Mcp::from_env(secrets.as_deref())?;
    mcp.initialize()?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir = format!("/data/local/tmp/megadaemon/state/inventory/{stamp}");
    eprintln!("[inv] directorio de captura en el móvil: {dir}");

    // --- 1. captura (por lotes: cada lote = una llamada root_exec) ---
    let mut ids: Vec<&str> = capture_plan().iter().map(|c| c.id).collect();
    ids.extend(copy_plan().iter().map(|c| c.id));
    // Los `du` son los caros (225 GB): van en lote propio y con más timeout.
    let mut batches: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for id in ids {
        if id.starts_with("du_") {
            if !current.is_empty() {
                batches.push(std::mem::take(&mut current));
            }
            batches.push(vec![id]);
        } else {
            current.push(id);
            if current.len() >= 8 {
                batches.push(std::mem::take(&mut current));
            }
        }
    }
    if !current.is_empty() {
        batches.push(current);
    }
    for (n, batch) in batches.iter().enumerate() {
        let cmd = build_capture_cmd(&dir, batch);
        let timeout = if batch.iter().any(|i| i.starts_with("du_")) {
            300_000
        } else {
            150_000
        };
        match mcp.root_exec_with(&cmd, timeout, 64 * 1024) {
            Ok(_) => eprintln!("[inv] lote {} ok ({} referencias)", n + 1, batch.len()),
            Err(e) => eprintln!("[inv] lote {} con avisos: {e}", n + 1),
        }
    }

    // --- digest del dispositivo (fuente de verdad del sha256) ---
    let out = mcp.root_exec_with(
        &format!("cd {dir} && sha256sum * 2>/dev/null; echo ---; ls -la {dir}"),
        60_000,
        128 * 1024,
    )?;
    let mut device_digests: Vec<(String, FileDigest)> = Vec::new();
    for line in out.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() >= 2 && f[0].len() == 64 && f[0].chars().all(|c| c.is_ascii_hexdigit()) {
            device_digests.push((
                f[1].trim_start_matches('*').to_string(),
                FileDigest {
                    sha256: f[0].to_string(),
                    bytes: 0,
                },
            ));
        }
    }
    eprintln!("[inv] ficheros capturados en el móvil: {}", device_digests.len());
    if device_digests.is_empty() {
        return Err("la captura no devolvió ningún SHA256; revisa el directorio".to_string());
    }

    // --- 2. traer por trozos + verificar ---
    let mut files: Vec<(String, String)> = Vec::new();
    let mut digests: Vec<(String, FileDigest)> = Vec::new();
    let refs_dir = format!("{out_dir}/refs");
    fs::create_dir_all(&refs_dir).map_err(|e| format!("mkdir {refs_dir}: {e}"))?;

    for (id, d) in &device_digests {
        let mut content: Vec<u8> = Vec::new();
        let mut offset = 0u64;
        loop {
            let cmd = format!(
                "tail -c +{} {dir}/{id} 2>/dev/null | head -c {CHUNK} | base64 -w 0 2>/dev/null || \
                 (tail -c +{} {dir}/{id} 2>/dev/null | head -c {CHUNK} | base64 | tr -d '\\n')",
                offset + 1,
                offset + 1
            );
            let b64 = mcp.root_exec(&cmd)?;
            let bytes = decode_b64(&b64);
            if bytes.is_empty() {
                break;
            }
            content.extend_from_slice(&bytes);
            offset += bytes.len() as u64;
            if (bytes.len() as u64) < CHUNK {
                break;
            }
        }
        let sha = sha256_hex(&content);
        let ok = sha == d.sha256;
        eprintln!(
            "[inv] {id}: {} bytes · sha256 {}",
            content.len(),
            if ok { "OK" } else { "DISCREPANCIA" }
        );
        if !ok {
            return Err(format!(
                "sha256 distinto en {id}: móvil {} vs local {sha}",
                d.sha256
            ));
        }
        let text = String::from_utf8_lossy(&content).to_string();
        fs::write(format!("{refs_dir}/{id}"), &content)
            .map_err(|e| format!("escritura {id}: {e}"))?;
        digests.push((
            id.clone(),
            FileDigest { sha256: sha, bytes: content.len() as u64 },
        ));
        files.push((id.clone(), text));
    }

    // --- 3. ensamblar ---
    let src = DeviceSource { files, digests };
    let inv = device_inventory::collect(&src, &Options::default());
    let manifest = manifest_ndjson(&src);

    fs::create_dir_all(&out_dir).map_err(|e| format!("mkdir {out_dir}: {e}"))?;
    let inv_path = format!("{out_dir}/inventory.json");
    let man_path = format!("{out_dir}/MANIFEST.ndjson");
    let mut f = fs::File::create(&inv_path).map_err(|e| format!("crear {inv_path}: {e}"))?;
    f.write_all(
        serde_json::to_string_pretty(&inv)
            .map_err(|e| format!("serializar inventario: {e}"))?
            .as_bytes(),
    )
        .map_err(|e| format!("escribir {inv_path}: {e}"))?;
    let mut f = fs::File::create(&man_path).map_err(|e| format!("crear {man_path}: {e}"))?;
    f.write_all(manifest.as_bytes())
        .map_err(|e| format!("escribir {man_path}: {e}"))?;

    // --- 4. resumen (sin volcar datos sensibles) ---
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
        "inventory": inv_path,
        "manifest": man_path,
        "refs": refs_dir,
        "referencias": inv["referencias"].as_array().map(|v| v.len()).unwrap_or(0),
        "apps_total": inv["apps"]["total"],
            "sha256_inventory": sha256_hex(fs::read(&inv_path).unwrap_or_default().as_slice()),
        }))
        .map_err(|e| format!("serializar resumen: {e}"))?
    );
    let _ = Duration::from_secs(0);
    Ok(())
}
