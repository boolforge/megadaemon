//! MegaDaemon v13.4 — Inventario de backup completo de chats.
//!
//! Recorre el state dir (y su subdirectorio `backup/`) y produce un
//! inventario por fichero con: origen, tamaño, SHA-256, si está cifrado, su
//! estado y **por qué**.
//!
//! ESTADOS (son la parte importante del módulo):
//!
//! * `exportable`        — se puede extraer el contenido útil ahora mismo.
//! * `encrypted_keystore`— el dato está cifrado con una clave que vive en el
//!   Android Keystore (hardware-backed). Se puede copiar el texto cifrado,
//!   **no** el contenido: sin la clave no hay backup utilizable.
//! * `needs_hook`        — hace falta interceptar el proceso de la app
//!   (Frida/Xposed) para llegar al contenido.
//! * `not_present`       — la fuente no está en el dispositivo.
//!
//! Lo que NO se hace aquí (y no se puede hacer sin hook):
//!
//! * WhatsApp: `msgstore.db` está cifrado con clave del keystore → sólo
//!   metadata (interlocutor, timestamps, tamaño) y ficheros multimedia.
//! * Telegram: `tdata/` está cifrado con AES-IGE y clave derivada del
//!   keystore → los `.db` de sesión sí son texto plano, pero sin el `auth_key`
//!   no se pueden reimportar.
//! * Signal: la base local está cifrada con clave del keystore; el `.backup`
//!   usa la contraseña de backup del usuario (que sí se puede usar).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Estado: el contenido se puede extraer sin más.
pub const STATUS_EXPORTABLE: &str = "exportable";
/// Estado: cifrado con clave del keystore → inservible sin la clave.
pub const STATUS_ENCRYPTED_KEYSTORE: &str = "encrypted_keystore";
/// Estado: hace falta hook en proceso.
pub const STATUS_NEEDS_HOOK: &str = "needs_hook";
/// Estado: la fuente no existe en el dispositivo.
pub const STATUS_NOT_PRESENT: &str = "not_present";

/// Subdirectorio de backup dentro del state dir.
pub const BACKUP_SUBDIR: &str = "backup";

/// Un elemento del inventario de backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupItem {
    /// Identificador estable (origen + hash de la ruta relativa).
    pub id: String,
    /// Origen lógico (`whatsapp`, `telegram`, `signal`, `discord`, `nostr`,
    /// `sms`, `generic`).
    pub source: String,
    /// Ruta absoluta en el dispositivo (o ruta esperada si no está presente).
    pub path: String,
    /// Tamaño en bytes.
    pub bytes: u64,
    /// SHA-256 en hexadecimal del fichero (vacío si no existe).
    pub sha256: String,
    /// Si el contenido está cifrado.
    pub encrypted: bool,
    /// Uno de `exportable|encrypted_keystore|needs_hook|not_present`.
    pub status: String,
    /// Explicación honesta de qué se puede y qué no se puede sacar.
    pub notes: String,
}

/// Fuentes que se buscan siempre, para poder reportar las que faltan.
const EXPECTED: [(&str, &str, bool, &str); 7] = [
    (
        "whatsapp",
        "/data/data/com.whatsapp/databases/msgstore.db",
        true,
        "Contenido cifrado con clave del keystore: sólo metadata (interlocutor, timestamps, \
         tamaño) y adjuntos en /sdcard/WhatsApp/Media. El texto de los mensajes exigiría \
         hook en proceso sobre com.whatsapp.",
    ),
    (
        "telegram",
        "/data/data/org.telegram.messenger/files/tdata",
        true,
        "tdata está cifrado (AES-IGE, clave derivada del keystore). Los .db de sesión local \
         son texto plano pero están atados al auth_key de la cuenta.",
    ),
    (
        "telegram_ngram",
        "/data/data/org.ngram/files/tdata",
        true,
        "Cliente Ngram (protocolo Telegram). Mismo análisis que Telegram: tdata cifrado, \
         .db de sesión en texto plano.",
    ),
    (
        "signal",
        "/data/data/org.thoughtcrime.securesms",
        true,
        "Base local cifrada con clave del keystore. El fichero .backup usa la contraseña de \
         backup (40 caracteres) del usuario, que sí permite restaurar.",
    ),
    (
        "discord",
        "/data/data/com.discord/app_localstorage/leveldb",
        false,
        "LevelDB legible con root: se copia y se parsea sin hook. Los tokens de sesión están \
         aparte (~/.dat) y no se incluyen en el backup.",
    ),
    (
        "nostr",
        "/sdcard/MegaDaemon/nostr",
        false,
        "Lista de relays y claves Nostr. Si hay nsec/secret, es la clave privada en claro: \
         sólo debe viajar dentro de la bóveda cifrada.",
    ),
    (
        "sms",
        "/sdcard/MegaDaemon/sms_export.json",
        false,
        "Export de content://sms (SMS+MMS). Requiere READ_SMS o root; el proveedor de \
         Android expone el historial en texto plano.",
    ),
];

/// Clasifica una ruta (relativa o absoluta) devolviendo
/// (origen, cifrado, estado, notas).
fn classify(path: &str) -> (&'static str, bool, &'static str, String) {
    let p = path.to_ascii_lowercase();

    // --- WhatsApp -----------------------------------------------------------
    if p.contains("whatsapp") {
        if p.contains("msgstore") || p.contains("databases") || p.contains("wa.db") {
            return (
                "whatsapp",
                true,
                STATUS_ENCRYPTED_KEYSTORE,
                "Cifrado con clave del Android Keystore (hardware-backed): se copia el \
                 texto cifrado, no el contenido. Lo único utilizable sin hook es la \
                 metadata (interlocutor, timestamps, tamaño) y los adjuntos de \
                 /sdcard/WhatsApp/Media. El texto de los mensajes requiere hooking \
                 en proceso (Frida/Xposed) sobre com.whatsapp.".to_string(),
            );
        }
        if p.contains("media") || p.contains(".jpg") || p.contains(".mp4") || p.contains(".opus") {
            return (
                "whatsapp",
                false,
                STATUS_EXPORTABLE,
                "Multimedia de WhatsApp en almacenamiento compartido: NO está cifrada, se \
                 copia tal cual. Ojo: contiene material sensible de terceros.".to_string(),
            );
        }
        return (
            "whatsapp",
            true,
            STATUS_ENCRYPTED_KEYSTORE,
            "PertenECE al dominio de WhatsApp: el contenido de la base está cifrado con \
             clave del keystore.".to_string(),
        );
    }

    // --- Telegram / Ngram ----------------------------------------------------
    if p.contains("telegram") || p.contains("tdata") || p.contains("ngram") {
        let src = if p.contains("ngram") { "telegram_ngram" } else { "telegram" };
        if p.contains("tdata") {
            return (
                src,
                true,
                STATUS_ENCRYPTED_KEYSTORE,
                "tdata de Telegram: AES-IGE con clave derivada del keystore (keystore-1). \
                 Copiar el fichero no sirve sin la clave; hace falta hookear el proceso \
                 para capturarla.".to_string(),
            );
        }
        if p.contains(".db") || p.contains("sqlite") || p.contains("map") {
            return (
                src,
                false,
                STATUS_EXPORTABLE,
                "SQLite de sesión local de Telegram: texto plano, copiable con root y \
                 legible con sqlite3. Ojo: está atado al auth_key de la cuenta, así que \
                 no se puede reimportar en otro cliente sin esa clave.".to_string(),
            );
        }
        return (
            src,
            false,
            STATUS_EXPORTABLE,
            "Recurso de Telegram copiable con root (telegram_photo, tdata_aux, etc.). \
             Si forma parte de tdata, el contenido real sigue cifrado.".to_string(),
        );
    }

    // --- Signal / Molly ------------------------------------------------------
    if p.contains("signal") || p.contains("securesms") || p.contains("molly") {
        if p.ends_with(".backup") {
            return (
                "signal",
                true,
                STATUS_EXPORTABLE,
                "Backup oficial de Signal: cifrado con la contraseña de backup del usuario \
                 (40 caracteres), NO con el keystore. Copiable con root y restaurable si \
                 se conoce esa contraseña.".to_string(),
            );
        }
        return (
            "signal",
            true,
            STATUS_ENCRYPTED_KEYSTORE,
            "Base local de Signal cifrada con clave del Android Keystore: sin esa clave el \
             fichero no se puede abrir. Hook en proceso para capturar la clave o el \
             contenido en claro.".to_string(),
        );
    }

    // --- Discord -------------------------------------------------------------
    if p.contains("discord") {
        if p.contains("leveldb") || p.contains(".ldb") || p.contains(".log") {
            return (
                "discord",
                false,
                STATUS_EXPORTABLE,
                "LevelDB de Discord: se copia con root y se parsea (sin hook) con un lector \
                 de LevelDB. Los tokens de sesión están en ~/.dat y NO se incluyen.".to_string(),
            );
        }
        return (
            "discord",
            false,
            STATUS_EXPORTABLE,
            "Recurso de Discord copiable con root. Las sesiones activas siguen siendo \
             sólo metadata: el texto cifrado del canal no está en estos ficheros.".to_string(),
        );
    }

    // --- Nostr ---------------------------------------------------------------
    if p.contains("nostr") || p.contains("nsec") {
        let sensitive = p.contains("nsec") || p.contains("secret") || p.contains("private");
        return (
            "nostr",
            sensitive,
            STATUS_EXPORTABLE,
            if sensitive {
                "Contiene material de clave privada Nostr en claro: exportable, pero sólo \
                 debe viajar dentro de la bóveda cifrada (backup_vault)."
                    .to_string()
            } else {
                "Configuración Nostr (relays, preferencias): exportable sin hook.".to_string()
            },
        );
    }

    // --- SMS / MMS -----------------------------------------------------------
    if p.contains("sms")
        || p.contains("mms")
        || p.contains("telephony")
        || p.contains("mmssms")
        || p.contains("content://sms")
    {
        return (
            "sms",
            false,
            STATUS_EXPORTABLE,
            "Export del historial SMS/MMS (content://sms). Requiere el permiso READ_SMS o \
             root; el proveedor de Android lo expone en texto plano, sin hook.".to_string(),
        );
    }

    // --- Genérico ------------------------------------------------------------
    (
        "generic",
        false,
        STATUS_EXPORTABLE,
        "Fichero genérico del state dir: se copia y se cifra en la bóveda tal cual.".to_string(),
    )
}

/// SHA-256 en hexadecimal de un buffer.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut s = String::with_capacity(64);
    for b in digest.iter() {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// SHA-256 en hexadecimal de un fichero (streaming).
pub fn sha256_file(path: &Path) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match f.read(&mut buf) {
            Ok(n) => n,
            Err(_) => return None,
        };
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(sha256_hex(&hasher.finalize()))
}

/// Recorre un directorio recogiendo ficheros regulares (sin seguir symlinks).
fn walk_files(root: &Path, out: &mut Vec<PathBuf>) {
    let rd = match fs::read_dir(root) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut entries: Vec<fs::DirEntry> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let md = match fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if md.file_type().is_symlink() {
            continue;
        }
        if md.is_dir() {
            walk_files(&p, out);
        } else if md.is_file() {
            out.push(p);
        }
    }
}

/// Construye el inventario completo de backup del `state_dir`.
///
/// Recorre el state dir y su subdirectorio `backup/` (sin duplicar los
/// ficheros que ya están dentro del primero) y añade una entrada
/// `not_present` por cada fuente conocida que no se encuentre, para que el
/// informe diga explícitamente qué falta y por qué.
pub fn inventory(state_dir: &str) -> Vec<BackupItem> {
    let mut items: Vec<BackupItem> = Vec::new();
    let root = Path::new(state_dir);

    if root.is_dir() {
        let mut files: Vec<PathBuf> = Vec::new();
        walk_files(root, &mut files);

        // Rutas relativas al state dir (para `backup/x` sale "backup/x").
        let mut seen: Vec<String> = Vec::new();
        for p in files {
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string();
            if seen.contains(&rel) {
                continue;
            }
            seen.push(rel.clone());

            let md = match fs::metadata(&p) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let (source, encrypted, status, notes) = classify(&rel);
            let sha = sha256_file(&p).unwrap_or_default();
            items.push(BackupItem {
                id: format!("{}-{}", source, &sha256_hex(rel.as_bytes())[..16]),
                source: source.to_string(),
                path: p.to_string_lossy().to_string(),
                bytes: md.len(),
                sha256: sha,
                encrypted,
                status: status.to_string(),
                notes,
            });
        }
    }

    // Fuentes ausentes.
    for (source, expected_path, encrypted, note) in EXPECTED.iter() {
        let found = items.iter().any(|i| &i.source == source);
        if found {
            continue;
        }
        items.push(BackupItem {
            id: format!("{}-missing", source),
            source: source.to_string(),
            path: expected_path.to_string(),
            bytes: 0,
            sha256: String::new(),
            encrypted: *encrypted,
            status: STATUS_NOT_PRESENT.to_string(),
            notes: format!("No encontrado en {}. {}.", state_dir, note),
        });
    }

    items.sort_by(|a, b| {
        a.source
            .cmp(&b.source)
            .then_with(|| a.path.cmp(&b.path))
    });
    items
}

/// Estado canónico de un elemento.
///
/// Si el elemento ya trae un estado de los cuatro canónicos, se respeta.
/// Si no, se deriva de la ruta y del flag `encrypted`.
pub fn status_of(item: &BackupItem) -> &str {
    let explicit = item.status.as_str();
    if matches!(
        explicit,
        STATUS_EXPORTABLE | STATUS_ENCRYPTED_KEYSTORE | STATUS_NEEDS_HOOK | STATUS_NOT_PRESENT
    ) {
        return explicit;
    }
    let (_src, _enc, status, _notes) = classify(&item.path);
    status
}

/// Resumen por estado y por origen (para informes y cockpit).
pub fn summarize(items: &[BackupItem]) -> serde_json::Value {
    use serde_json::json;
    let mut by_status = serde_json::Map::new();
    let mut by_source = serde_json::Map::new();
    let mut total_bytes: u64 = 0;
    for it in items {
        let st = status_of(it).to_string();
        let e = by_status.entry(st.clone()).or_insert(json!(0));
        *e = json!(e.as_u64().unwrap_or(0) + 1);
        let s = by_source.entry(it.source.clone()).or_insert(json!(0));
        *s = json!(s.as_u64().unwrap_or(0) + 1);
        total_bytes += it.bytes;
    }
    json!({
        "items": items.len(),
        "total_bytes": total_bytes,
        "by_status": serde_json::Value::Object(by_status),
        "by_source": serde_json::Value::Object(by_source),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmpdir(tag: &str) -> String {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "md_backupfull_{}_{}_{}",
            tag,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&p).unwrap();
        p.to_string_lossy().to_string()
    }

    #[test]
    fn inventory_detecta_whatsapp_y_marca_keystore() {
        let dir = tmpdir("wa");
        let dbdir = format!("{}/com.whatsapp/databases", dir);
        fs::create_dir_all(&dbdir).unwrap();
        fs::write(format!("{}/msgstore.db", dbdir), b"SQLite format 3\0fake").unwrap();

        let items = inventory(&dir);
        let wa = items.iter().find(|i| i.source == "whatsapp").expect("whatsapp");
        assert_eq!(wa.status, STATUS_ENCRYPTED_KEYSTORE);
        assert!(wa.encrypted);
        assert!(wa.notes.contains("keystore"));
        assert_eq!(wa.bytes, 18);
        assert_eq!(wa.sha256.len(), 64);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn inventory_marca_como_ausentes_las_fuentes_no_encontradas() {
        let dir = tmpdir("vacio");
        let items = inventory(&dir);
        assert!(items
            .iter()
            .any(|i| i.source == "signal" && i.status == STATUS_NOT_PRESENT));
        assert!(items
            .iter()
            .any(|i| i.source == "nostr" && i.status == STATUS_NOT_PRESENT));
        // Todos los estados deben estar dentro del conjunto canónico.
        for it in &items {
            assert!(matches!(
                it.status.as_str(),
                STATUS_EXPORTABLE
                    | STATUS_ENCRYPTED_KEYSTORE
                    | STATUS_NEEDS_HOOK
                    | STATUS_NOT_PRESENT
            ));
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn inventory_incluye_el_subdirectorio_backup() {
        let dir = tmpdir("bdir");
        let bdir = format!("{}/{}", dir, BACKUP_SUBDIR);
        fs::create_dir_all(&bdir).unwrap();
        fs::write(format!("{}/sms_export.json", bdir), b"[]").unwrap();
        let items = inventory(&dir);
        let sms = items.iter().find(|i| i.source == "sms").expect("sms");
        assert_eq!(sms.status, STATUS_EXPORTABLE);
        assert!(sms.path.contains(BACKUP_SUBDIR));
        // El fichero no debe aparecer duplicado.
        assert_eq!(items.iter().filter(|i| i.source == "sms").count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn status_of_deriva_cuando_el_estado_no_es_canonico() {
        let mut item = BackupItem {
            id: "x".into(),
            source: "whatsapp".into(),
            path: "/data/data/com.whatsapp/databases/msgstore.db".into(),
            bytes: 10,
            sha256: String::new(),
            encrypted: true,
            status: "inventado".into(),
            notes: String::new(),
        };
        assert_eq!(status_of(&item), STATUS_ENCRYPTED_KEYSTORE);
        // Si el estado es canónico, manda el explícito.
        item.status = STATUS_NOT_PRESENT.to_string();
        assert_eq!(status_of(&item), STATUS_NOT_PRESENT);
    }

    #[test]
    fn classify_distingue_tdata_de_los_db_planos() {
        let (_s, enc, st, _n) = classify("/data/data/org.telegram.messenger/files/tdata/D877");
        assert!(enc);
        assert_eq!(st, STATUS_ENCRYPTED_KEYSTORE);

        let (_s, enc, st, _n) = classify("/data/data/org.telegram.messenger/cache/db.sqlite");
        assert!(!enc);
        assert_eq!(st, STATUS_EXPORTABLE);
    }
}
