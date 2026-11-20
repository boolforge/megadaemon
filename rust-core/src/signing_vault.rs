//! Vault de firma (`signing_vault`).
//!
//! QUÉ ES (especificación del usuario, "compartir los archivos de firma
//! sincronizada a todas las instancias"):
//!
//!   * ZONA PÚBLICA: certificados, CMS/CAdES, PDFs PAdES-B-B, `afirma://`.
//!     Es lo que se REPLICA a todas las instancias de la flota.
//!   * ZONA PRIVADA: material que NO sale del dispositivo (claves privadas,
//!     tokens, material de firma personal).  Nunca se exporta, ni siquiera
//!     por error: el export es filtrado por zona y hay un test que lo fija.
//!   * MANIFIESTO NDJSON indexado: una línea por fichero, con el sha256
//!     del contenido.  En destino el sha256 se VERIFICA antes de aceptar
//!     nada (fail-closed por entrada).
//!   * Tool MCP `signing_vault` (list/put/get/verify + export/import) y
//!     ruta `/api/signing_vault`.
//!
//! DÓNDE VIVE:
//!   <state_dir>/vault/public/<nombre>     <- zona replicable
//!   <state_dir>/vault/private/<nombre>    <- zona local, no se replica
//!   <state_dir>/vault/manifest.ndjson     <- índice (una línea por fichero)
//!
//! POR QUÉ ES AUTOCONTENIDO:
//!   Este módulo se compila también como arnés de pruebas con
//!   `#[path = "../src/signing_vault.rs"] mod signing_vault;` (el mismo
//!   patrón que `tests/signing.rs`), así que NO puede depender de `crate::`
//!   ni de otros módulos de la biblioteca: usa `sha2`, `base64ct` y
//!   `serde_json`, que ya son dependencias del crate.
//!
//! LÍMITE HONESTO: esta bóveda NO implementa firma criptográfica (CMS/PAdES
//! las hace `signing.rs`); mueve, indexa y VERIFICA material ya firmado.
//! Si algún día se le pide firmar, el signer va en `signing.rs` y aquí sólo
//! se guarda el resultado.

use base64ct::Encoding;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Nombre del directorio de la bóveda dentro del `state_dir`.
pub const VAULT_DIR: &str = "vault";
/// Nombre del manifiesto NDJSON.
pub const MANIFEST_NAME: &str = "manifest.ndjson";
/// Longitud máxima del nombre de un fichero en la bóveda.
pub const MAX_NAME_LEN: usize = 128;

/// Zona de la bóveda.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zone {
    /// Se replica a todas las instancias.
    Public,
    /// Nunca sale del dispositivo.
    Private,
}

impl Zone {
    /// Nombre estable de la zona (en el manifiesto y en la API).
    pub fn as_str(&self) -> &'static str {
        match self {
            Zone::Public => "public",
            Zone::Private => "private",
        }
    }

    /// Subdirectorio de la zona.
    pub fn dir(&self) -> &'static str {
        match self {
            Zone::Public => "public",
            Zone::Private => "private",
        }
    }

    /// Parsea el nombre de zona (`public` / `private`).
    pub fn parse(s: &str) -> Option<Zone> {
        match s.trim().to_ascii_lowercase().as_str() {
            "public" | "pub" => Some(Zone::Public),
            "private" | "priv" => Some(Zone::Private),
            _ => None,
        }
    }
}

/// Entrada del manifiesto: un fichero dentro de la bóveda.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VaultEntry {
    /// Nombre del fichero (sin ruta).
    pub name: String,
    /// Zona: "public" o "private".
    pub zone: String,
    /// SHA-256 del contenido, en hexadecimal.
    pub sha256: String,
    /// Tamaño del contenido en bytes.
    pub size_bytes: u64,
    /// Tipo MIME (informativo).
    pub mime: String,
    /// Procedencia (informativo): quién lo puso.
    pub origin: String,
    /// Fecha de alta (epoch segundos).
    pub ts: u64,
}

/// Resultado de verificar una entrada contra su manifiesto.
#[derive(Clone, Debug, Serialize)]
pub struct VerifyReport {
    pub name: String,
    pub zone: String,
    pub ok: bool,
    /// Motivo del fallo (vacío si todo va bien).
    pub reason: String,
    /// SHA-256 declarado en el manifiesto.
    pub expected_sha256: String,
    /// SHA-256 recalculado sobre el fichero en disco.
    pub actual_sha256: String,
}

/// Resultado de importar un paquete de la zona pública.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ImportReport {
    /// Entradas aceptadas (sha-256 verificado en destino).
    pub imported: usize,
    /// Entradas rechazadas.
    pub rejected: usize,
    /// Detalle de cada rechazo.
    pub errors: Vec<String>,
}

/// Bytes a hexadecimal en minúsculas.
pub fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// SHA-256 en hexadecimal.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex(&hasher.finalize())
}

/// Epoch actual (segundos).
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Valida el nombre de un fichero: sin rutas, sin `..`, sin separadores.
///
/// Es la única barrera contra el path traversal: todo lo que entre por
/// `put`/`get` pasa por aquí antes de tocar el disco.
pub fn sanitize_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("nombre vacio".to_string());
    }
    if n.len() > MAX_NAME_LEN {
        return Err(format!("nombre demasiado largo ({} > {})", n.len(), MAX_NAME_LEN));
    }
    if !n
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(format!("nombre invalido: {}", n));
    }
    if n.starts_with('.') || n.contains("..") {
        return Err(format!("nombre invalido: {}", n));
    }
    Ok(n.to_string())
}

/// La bóveda de firma.
pub struct SigningVault {
    root: PathBuf,
}

impl SigningVault {
    /// Abre (o crea) la bóveda bajo `state_dir`.
    pub fn new(state_dir: &Path) -> Result<Self, String> {
        let root = state_dir.join(VAULT_DIR);
        fs::create_dir_all(root.join(Zone::Public.dir())).map_err(|e| e.to_string())?;
        fs::create_dir_all(root.join(Zone::Private.dir())).map_err(|e| e.to_string())?;
        Ok(SigningVault { root })
    }

    /// Raíz de la bóveda.
    #[allow(dead_code)]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Ruta del manifiesto.
    pub fn manifest_path(&self) -> PathBuf {
        self.root.join(MANIFEST_NAME)
    }

    /// Ruta del fichero dentro de la zona.
    pub fn entry_path(&self, zone: Zone, name: &str) -> PathBuf {
        self.root.join(zone.dir()).join(name)
    }

    /// Lee el manifiesto.  Las líneas rotas se ignoran (no se pierde el
    /// resto del índice), pero se cuentan para poder avisar.
    pub fn read_manifest(&self) -> Result<Vec<VaultEntry>, String> {
        let path = self.manifest_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<VaultEntry>(t) {
                out.push(entry);
            }
        }
        Ok(out)
    }

    /// Escribe el manifiesto completo, ordenado de forma determinista
    /// (zona, nombre) para que dos ejecuciones den el mismo fichero.
    pub fn write_manifest(&self, entries: &[VaultEntry]) -> Result<(), String> {
        let mut sorted: Vec<&VaultEntry> = entries.iter().collect();
        sorted.sort_by(|a, b| (a.zone.as_str(), a.name.as_str()).cmp(&(b.zone.as_str(), b.name.as_str())));
        let mut text = String::new();
        for e in sorted {
            let line = serde_json::to_string(e).map_err(|e| e.to_string())?;
            text.push_str(&line);
            text.push('\n');
        }
        let path = self.manifest_path();
        let tmp = self.root.join(".manifest.ndjson.tmp");
        {
            let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
            f.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())?;
        }
        fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Guarda un fichero en la bóveda y lo indexa.
    ///
    /// Es idempotente: guardar dos veces el mismo nombre con el mismo
    /// contenido deja UNA entrada en el manifiesto.
    pub fn put(
        &self,
        name: &str,
        zone: Zone,
        content: &[u8],
        mime: &str,
        origin: &str,
    ) -> Result<VaultEntry, String> {
        let safe = sanitize_name(name)?;
        let dir = self.root.join(zone.dir());
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(&safe);
        let tmp = dir.join(format!(".{}.tmp", safe));
        {
            let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
            f.write_all(content).map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())?;
        }
        fs::rename(&tmp, &path).map_err(|e| e.to_string())?;

        let entry = VaultEntry {
            name: safe.clone(),
            zone: zone.as_str().to_string(),
            sha256: sha256_hex(content),
            size_bytes: content.len() as u64,
            mime: mime.to_string(),
            origin: origin.to_string(),
            ts: now(),
        };
        let mut entries = self.read_manifest()?;
        entries.retain(|e| !(e.name == safe && e.zone == zone.as_str()));
        entries.push(entry.clone());
        self.write_manifest(&entries)?;
        Ok(entry)
    }

    /// Recupera un fichero.  VERIFICA el sha-256 contra el manifiesto:
    /// si el contenido en disco ha cambiado, se devuelve error (fail-closed).
    pub fn get(&self, name: &str) -> Result<(VaultEntry, Vec<u8>), String> {
        let safe = sanitize_name(name)?;
        let entries = self.read_manifest()?;
        let entry = entries
            .iter()
            .find(|e| e.name == safe)
            .ok_or_else(|| format!("no existe en la boveda: {}", safe))?
            .clone();
        let zone = Zone::parse(&entry.zone).ok_or_else(|| format!("zona invalida: {}", entry.zone))?;
        let data = fs::read(self.entry_path(zone, &safe)).map_err(|e| e.to_string())?;
        let actual = sha256_hex(&data);
        if actual != entry.sha256 {
            return Err(format!(
                "sha256 no coincide (manifiesto {} vs en disco {}): {}",
                entry.sha256, actual, safe
            ));
        }
        Ok((entry, data))
    }

    /// Lista el índice (opcionalmente, sólo una zona).
    pub fn list(&self, zone: Option<Zone>) -> Result<Vec<VaultEntry>, String> {
        let mut entries = self.read_manifest()?;
        if let Some(z) = zone {
            entries.retain(|e| e.zone == z.as_str());
        }
        entries.sort_by(|a, b| (a.zone.as_str(), a.name.as_str()).cmp(&(b.zone.as_str(), b.name.as_str())));
        Ok(entries)
    }

    /// Verifica una entrada: recalcula el sha-256 del fichero y lo compara
    /// con el del manifiesto.
    pub fn verify(&self, name: &str) -> Result<VerifyReport, String> {
        let safe = sanitize_name(name)?;
        let entries = self.read_manifest()?;
        let entry = entries
            .iter()
            .find(|e| e.name == safe)
            .ok_or_else(|| format!("no existe en la boveda: {}", safe))?
            .clone();
        let zone = Zone::parse(&entry.zone).ok_or_else(|| format!("zona invalida: {}", entry.zone))?;
        let path = self.entry_path(zone, &safe);
        if !path.exists() {
            return Ok(VerifyReport {
                name: safe,
                zone: entry.zone.clone(),
                ok: false,
                reason: "fichero ausente".to_string(),
                expected_sha256: entry.sha256.clone(),
                actual_sha256: String::new(),
            });
        }
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        let actual = sha256_hex(&data);
        let ok = actual == entry.sha256;
        Ok(VerifyReport {
            name: safe,
            zone: entry.zone.clone(),
            ok,
            reason: if ok { String::new() } else { "sha256 no coincide".to_string() },
            expected_sha256: entry.sha256.clone(),
            actual_sha256: actual,
        })
    }

    /// Exporta la ZONA PÚBLICA como paquete NDJSON en base64.
    ///
    /// La zona privada queda fuera por construccion (filtro por zona) y hay
    /// un test que lo comprueba: nada de la zona privada sale por aquí.
    pub fn export_public(&self) -> Result<String, String> {
        let entries = self.read_manifest()?;
        let mut text = String::new();
        for e in entries.iter().filter(|e| e.zone == Zone::Public.as_str()) {
            let zone = Zone::Public;
            let data = fs::read(self.entry_path(zone, &e.name)).map_err(|e| e.to_string())?;
            let line = json!({
                "name": e.name,
                "sha256": e.sha256,
                "size_bytes": e.size_bytes,
                "mime": e.mime,
                "origin": e.origin,
                "ts": e.ts,
                "content_b64": base64ct::Base64::encode_string(&data),
            });
            text.push_str(&serde_json::to_string(&line).map_err(|e| e.to_string())?);
            text.push('\n');
        }
        Ok(base64ct::Base64::encode_string(text.as_bytes()))
    }

    /// Importa un paquete de la zona pública.
    ///
    /// En destino se VERIFICA el sha-256 de cada entrada ANTES de escribir
    /// nada: la que no cuadra se rechaza (fail-closed por entrada) y no toca
    /// el disco.  Sólo se escribe en la zona pública.
    pub fn import_public(&self, bundle_b64: &str) -> Result<ImportReport, String> {
        let raw = base64ct::Base64::decode_vec(bundle_b64.trim())
            .map_err(|e| format!("base64 invalido: {}", e))?;
        let text = String::from_utf8(raw).map_err(|e| format!("no es utf-8: {}", e))?;
        let mut report = ImportReport::default();
        let mut entries = self.read_manifest()?;
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            let v: Value = match serde_json::from_str(t) {
                Ok(v) => v,
                Err(e) => {
                    report.rejected += 1;
                    report.errors.push(format!("linea no json: {}", e));
                    continue;
                }
            };
            let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let declared = v.get("sha256").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let content_b64 = v.get("content_b64").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let safe = match sanitize_name(&name) {
                Ok(s) => s,
                Err(e) => {
                    report.rejected += 1;
                    report.errors.push(format!("{}: {}", name, e));
                    continue;
                }
            };
            let data = match base64ct::Base64::decode_vec(&content_b64) {
                Ok(d) => d,
                Err(e) => {
                    report.rejected += 1;
                    report.errors.push(format!("{}: base64 invalido: {}", safe, e));
                    continue;
                }
            };
            let actual = sha256_hex(&data);
            if actual != declared {
                report.rejected += 1;
                report.errors.push(format!(
                    "{}: sha256 no coincide (declarado {}, real {})",
                    safe, declared, actual
                ));
                continue;
            }
            // Aceptada: se escribe en la zona PÚBLICA y se indexa.
            let mime = v.get("mime").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let origin = v.get("origin").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let entry = self.put(&safe, Zone::Public, &data, &mime, &origin)?;
            entries.retain(|e| !(e.name == entry.name && e.zone == entry.zone));
            entries.push(entry);
            report.imported += 1;
        }
        self.write_manifest(&entries)?;
        Ok(report)
    }
}

/// Punto de entrada del tool MCP `signing_vault`.
///
/// Acciones: `list`, `put`, `get`, `verify`, `export_public`, `import_public`.
///
/// Devuelve siempre JSON: `{"ok": true, ...}` o `{"ok": false, "error": ...}`.
/// Un error es un dato, no un panic.
///
/// La zona es OBLIGATORIA en `put`: la zona pública es la que se replica, así
/// que darla por buena ante la duda podría sacar material privado del
/// dispositivo. Si no viene, se rechaza.
pub fn handle_action(action: &str, args: &Value, state_dir: &Path) -> Value {
    let vault = match SigningVault::new(state_dir) {
        Ok(v) => v,
        Err(e) => return json!({"ok": false, "error": e}),
    };
    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
    match action {
        "list" => {
            let zone = args
                .get("zone")
                .and_then(|v| v.as_str())
                .and_then(Zone::parse);
            match vault.list(zone) {
                Ok(entries) => json!({"ok": true, "entries": entries, "count": entries.len()}),
                Err(e) => json!({"ok": false, "error": e}),
            }
        }
        "put" => {
            let zone = match args.get("zone").and_then(|v| v.as_str()) {
                Some(z) => match Zone::parse(z) {
                    Some(zone) => zone,
                    None => {
                        return json!({"ok": false, "error": format!("zona invalida: {}", z)})
                    }
                },
                None => {
                    return json!({
                        "ok": false,
                        "error": "falta 'zone' (public = replicable, private = local, nunca sale)",
                    })
                }
            };
            let content = match args.get("content_b64").and_then(|v| v.as_str()) {
                Some(b64) => match base64ct::Base64::decode_vec(b64) {
                    Ok(d) => d,
                    Err(e) => return json!({"ok": false, "error": format!("base64 invalido: {}", e)}),
                },
                None => Vec::new(),
            };
            let mime = args.get("mime").and_then(|v| v.as_str()).unwrap_or("");
            let origin = args.get("origin").and_then(|v| v.as_str()).unwrap_or("");
            match vault.put(name, zone, &content, mime, origin) {
                Ok(entry) => json!({"ok": true, "entry": entry}),
                Err(e) => json!({"ok": false, "error": e}),
            }
        }
        "get" => match vault.get(name) {
            Ok((entry, data)) => json!({
                "ok": true,
                "entry": entry,
                "content_b64": base64ct::Base64::encode_string(&data),
            }),
            Err(e) => json!({"ok": false, "error": e}),
        },
        "verify" => match vault.verify(name) {
            Ok(report) => json!({"ok": report.ok, "report": report}),
            Err(e) => json!({"ok": false, "error": e}),
        },
        "export_public" => match vault.export_public() {
            Ok(bundle) => json!({"ok": true, "bundle_b64": bundle}),
            Err(e) => json!({"ok": false, "error": e}),
        },
        "import_public" => {
            let bundle = args.get("bundle_b64").and_then(|v| v.as_str()).unwrap_or("");
            match vault.import_public(bundle) {
                Ok(report) => json!({"ok": true, "report": report}),
                Err(e) => json!({"ok": false, "error": e}),
            }
        }
        other => json!({"ok": false, "error": format!("accion desconocida: {}", other)}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "md_signing_vault_{}_{}_{}",
            tag,
            std::process::id(),
            n
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn put_get_roundtrip() {
        let dir = temp_dir("roundtrip");
        let vault = SigningVault::new(&dir).unwrap();
        let data = b"-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n";
        let entry = vault
            .put("cert.pem", Zone::Public, data, "application/x-pem-file", "test")
            .unwrap();
        assert_eq!(entry.zone, "public");
        assert_eq!(entry.size_bytes, data.len() as u64);
        assert_eq!(entry.sha256, sha256_hex(data));
        let (got_entry, got) = vault.get("cert.pem").unwrap();
        assert_eq!(got, data.to_vec());
        assert_eq!(got_entry.sha256, entry.sha256);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_detects_tampering() {
        let dir = temp_dir("tamper");
        let vault = SigningVault::new(&dir).unwrap();
        vault.put("a.bin", Zone::Public, b"contenido original", "", "").unwrap();
        let ok = vault.verify("a.bin").unwrap();
        assert!(ok.ok, "deberia estar ok: {:?}", ok);
        // Se manipula el fichero por fuera de la boveda.
        fs::write(vault.entry_path(Zone::Public, "a.bin"), b"manipulado").unwrap();
        let bad = vault.verify("a.bin").unwrap();
        assert!(!bad.ok);
        assert_eq!(bad.reason, "sha256 no coincide");
        assert_eq!(bad.actual_sha256, sha256_hex(b"manipulado"));
        // Y get() es fail-closed: no devuelve el contenido manipulado.
        assert!(vault.get("a.bin").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn private_zone_is_never_exported() {
        let dir = temp_dir("private");
        let vault = SigningVault::new(&dir).unwrap();
        vault.put("pub.pem", Zone::Public, b"publico", "", "").unwrap();
        vault
            .put("priv.key", Zone::Private, b"CLAVE PRIVADA", "", "")
            .unwrap();
        let bundle = vault.export_public().unwrap();
        let decoded = base64ct::Base64::decode_vec(&bundle).unwrap();
        let text = String::from_utf8(decoded).unwrap();
        assert!(text.contains("pub.pem"), "debe exportar la zona publica");
        assert!(!text.contains("priv.key"), "NO debe exportar la zona privada");
        assert!(!text.contains("CLAVE PRIVADA"), "NO debe exportar contenido privado");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_verifies_sha256_at_destination() {
        let dir = temp_dir("import");
        let src = SigningVault::new(&dir).unwrap();
        src.put("x.p7s", Zone::Public, b"contenido", "", "").unwrap();
        let bundle = src.export_public().unwrap();

        // Destino: el mismo bundle (sha-256 correcto) -> se acepta.
        let dst_dir = temp_dir("import_dst");
        let dst = SigningVault::new(&dst_dir).unwrap();
        let report = dst.import_public(&bundle).unwrap();
        assert_eq!(report.imported, 1);
        assert_eq!(report.rejected, 0);
        let (_e, data) = dst.get("x.p7s").unwrap();
        assert_eq!(data, b"contenido".to_vec());

        // Destino: bundle manipulado (sha-256 falso) -> se RECHAZA y no
        // escribe nada en disco.  El paquete es base64: hay que decodificar,
        // tocar el NDJSON de dentro y volver a codificar.
        let bad_dir = temp_dir("import_bad");
        let bad = SigningVault::new(&bad_dir).unwrap();
        let decoded = base64ct::Base64::decode_vec(&bundle).unwrap();
        let text = String::from_utf8(decoded).unwrap();
        let tampered_text = text.replace("\"sha256\":\"", "\"sha256\":\"0");
        assert_ne!(tampered_text, text, "el bundle debe quedar manipulado");
        let tampered = base64ct::Base64::encode_string(tampered_text.as_bytes());
        let report2 = bad.import_public(&tampered).unwrap();
        assert_eq!(report2.imported, 0);
        assert_eq!(report2.rejected, 1);
        assert!(!report2.errors.is_empty());
        assert!(!bad.entry_path(Zone::Public, "x.p7s").exists());
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&dst_dir);
        let _ = fs::remove_dir_all(&bad_dir);
    }

    #[test]
    fn names_reject_traversal() {
        let dir = temp_dir("names");
        let vault = SigningVault::new(&dir).unwrap();
        assert!(vault.put("../evil", Zone::Public, b"x", "", "").is_err());
        assert!(vault.put("a/b", Zone::Public, b"x", "", "").is_err());
        assert!(vault.put(".hidden", Zone::Public, b"x", "", "").is_err());
        assert!(vault.put("", Zone::Public, b"x", "", "").is_err());
        assert!(vault.put("ok_name-1.pem", Zone::Public, b"x", "", "").is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn manifest_is_deterministic_and_idempotent() {
        let dir = temp_dir("manifest");
        let vault = SigningVault::new(&dir).unwrap();
        vault.put("b.txt", Zone::Public, b"b", "", "").unwrap();
        vault.put("a.txt", Zone::Public, b"a", "", "").unwrap();
        let first = fs::read(vault.manifest_path()).unwrap();
        // Mismo contenido, otro orden de escritura -> mismo manifiesto.
        let dir2 = temp_dir("manifest2");
        let vault2 = SigningVault::new(&dir2).unwrap();
        vault2.put("a.txt", Zone::Public, b"a", "", "").unwrap();
        vault2.put("b.txt", Zone::Public, b"b", "", "").unwrap();
        let second = fs::read(vault2.manifest_path()).unwrap();
        // El manifiesto es NDJSON de las entradas ORDENADAS, y cada entrada
        // lleva su `ts` (epoch SEGUNDOS): dos bovedas creadas en segundos
        // distintos NO pueden ser byte-a-byte iguales (el test era flaky
        // por reloj: en la suite completa las dos creaciones caian en
        // segundos distintos y el test era rojo; en solitario pasaba).
        // Lo que se quiere comprobar es que el CONTENIDO y el ORDEN son
        // deterministas => se comparan las entradas sin `ts`.
        let sin_ts = |bytes: &[u8]| -> Vec<serde_json::Value> {
            String::from_utf8_lossy(bytes)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    let mut v: serde_json::Value =
                        serde_json::from_str(l).expect("cada linea del manifiesto es JSON");
                    if let Some(o) = v.as_object_mut() {
                        o.remove("ts");
                    }
                    v
                })
                .collect()
        };
        assert_eq!(
            sin_ts(&first),
            sin_ts(&second),
            "el manifiesto debe ser determinista (mismo contenido y mismo orden)"
        );

        // Re-put del mismo nombre -> una sola entrada (idempotente).
        vault.put("a.txt", Zone::Public, b"a2", "", "").unwrap();
        let entries = vault.list(None).unwrap();
        let count = entries.iter().filter(|e| e.name == "a.txt").count();
        assert_eq!(count, 1, "no debe duplicar entradas");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&dir2);
    }

    #[test]
    fn list_filters_by_zone() {
        let dir = temp_dir("zones");
        let vault = SigningVault::new(&dir).unwrap();
        vault.put("p1", Zone::Public, b"a", "", "").unwrap();
        vault.put("p2", Zone::Private, b"b", "", "").unwrap();
        assert_eq!(vault.list(Some(Zone::Public)).unwrap().len(), 1);
        assert_eq!(vault.list(Some(Zone::Private)).unwrap().len(), 1);
        assert_eq!(vault.list(None).unwrap().len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_action_never_panics() {
        let dir = temp_dir("action");
        let out = handle_action("accion_inventada", &json!({}), &dir);
        assert_eq!(out.get("ok").and_then(|v| v.as_bool()), Some(false));
        let out2 = handle_action("list", &json!({"zone": "public"}), &dir);
        assert_eq!(out2.get("ok").and_then(|v| v.as_bool()), Some(true));
        // put + get + verify por la via del tool.
        let b64 = base64ct::Base64::encode_string(b"hola");
        let put = handle_action("put", &json!({"name":"t.txt","zone":"public","content_b64":b64}), &dir);
        assert_eq!(put.get("ok").and_then(|v| v.as_bool()), Some(true));
        let verify = handle_action("verify", &json!({"name":"t.txt"}), &dir);
        assert_eq!(verify.get("ok").and_then(|v| v.as_bool()), Some(true));
        // put SIN zone -> fail-closed: la zona publica es la que se replica.
        let nozone = handle_action("put", &json!({"name":"z.txt","content_b64": b64}), &dir);
        assert_eq!(nozone.get("ok").and_then(|v| v.as_bool()), Some(false));
        let _ = fs::remove_dir_all(&dir);
    }
}
