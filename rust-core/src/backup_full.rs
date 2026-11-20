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

use crate::util_fs::walk_files;
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

// ---------------------------------------------------------------------------
// W3 · Backup ultra completo: niveles, zonas y plan de pack
// ---------------------------------------------------------------------------

/// Nivel del backup.
///
/// La separación importa: lo volátil se regenera, el ROM es material de
/// fábrica, los datos de usuario son lo que el usuario nota si se pierde, y
/// las credenciales son lo único que **no puede** viajar en claro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    /// L0 · volátil: state dir, tablón, journal, audit, config de la UI.
    L0Volatile,
    /// L1 · ROM: boot/init/etc, particiones de sólo lectura y el mundo seguro.
    L1Rom,
    /// L2 · datos de usuario: DCIM, Downloads, Documents, /sdcard, apps.
    L2UserData,
    /// L3 · credenciales: sólo cifradas, con clave ligada al hardware.
    L3Credentials,
}

impl Level {
    /// Nombre estable (`l0`, `l1`, `l2`, `l3`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::L0Volatile => "l0",
            Level::L1Rom => "l1",
            Level::L2UserData => "l2",
            Level::L3Credentials => "l3",
        }
    }

    /// Título legible.
    pub fn title(&self) -> &'static str {
        match self {
            Level::L0Volatile => "L0 · volátil (estado del daemon)",
            Level::L1Rom => "L1 · ROM y mundo seguro",
            Level::L2UserData => "L2 · datos de usuario",
            Level::L3Credentials => "L3 · credenciales (sólo cifradas)",
        }
    }

    /// Los cuatro niveles, en orden.
    pub fn all() -> [Level; 4] {
        [
            Level::L0Volatile,
            Level::L1Rom,
            Level::L2UserData,
            Level::L3Credentials,
        ]
    }
}

/// Zona de un dato.
///
/// * `Public`  — se replica a **todas** las instancias (con SHA-256 verificado
///   en el destino): certificados, CMS, PDF firmados, el material público de
///   firma.
/// * `Private` — **nunca sale del dispositivo**: claves privadas, tokens
///   PKCS#11, material ligado al hardware.
///
/// Los nombres coinciden con los de `signing_vault::Zone` (`public`/`private`)
/// para que los manifiestos de los dos módulos se entiendan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Zone {
    /// Se replica a todas las instancias.
    Public,
    /// Nunca sale del dispositivo.
    Private,
}

impl Zone {
    /// Nombre estable (`public`/`private`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Zone::Public => "public",
            Zone::Private => "private",
        }
    }
}

/// Las **7 particiones reales** del mundo seguro en este firmware.
///
/// `init_boot`, `vbmeta_vendor` y `sec` **no existen** aquí (la cadena AVB
/// tiene 2 niveles, no 3): inventarlas sería prometer un backup imposible.
pub const EFS_SAFE_WORLD: [&str; 7] = [
    "sec_efs",
    "keystorage",
    "keyrefuge",
    "uh",
    "uhcfg",
    "spu",
    "prism",
];

/// Particiones que se agradecerían en un volcado pero **no existen** en este
/// firmware.
pub const EFS_NON_EXISTENT: [&str; 3] = ["init_boot", "vbmeta_vendor", "sec"];

/// Algo que **no** es recuperable, con el motivo.
///
/// Un backup sin esto miente: dice "está respaldado" cuando en realidad sólo
/// se ha copiado un ciphertext que sin la clave no sirve para nada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotRecoverable {
    /// Qué es.
    pub what: String,
    /// Por qué no se puede recuperar.
    pub reason: String,
    /// Qué sí se puede sacar de él.
    pub salvage: String,
}

/// Lo que no se puede recuperar de este móvil, declarado explícitamente.
pub fn not_recoverable() -> Vec<NotRecoverable> {
    vec![
        NotRecoverable {
            what: "WhatsApp · msgstore.db (texto de los mensajes)".into(),
            reason: "Cifrado con clave del Android Keystore (hardware-backed): la clave no \
                    sale del TEE sin credenciales de usuario."
                .into(),
            salvage: "metadata (interlocutor, timestamps, tamaño) y multimedia de \
                      /sdcard/WhatsApp/Media; el texto exigiría hook en proceso."
                .into(),
        },
        NotRecoverable {
            what: "Telegram · tdata/ (sesiones y claves de cuenta)".into(),
            reason: "AES-IGE con clave derivada del keystore; sin `auth_key` los `.db` de \
                    sesión no se pueden reimportar."
                .into(),
            salvage: "los `.db` de sesión local en texto plano y los adjuntos de /sdcard."
                .into(),
        },
        NotRecoverable {
            what: "Signal · base local cifrada".into(),
            reason: "Clave en el Android Keystore; el `.backup` oficial sí es recuperable \
                    porque usa la contraseña de backup del usuario."
                .into(),
            salvage: "el fichero `.backup` oficial (con su contraseña) y los adjuntos."
                .into(),
        },
        NotRecoverable {
            what: "keystore2 / AndroidKeyStore · material de clave del sistema".into(),
            reason: "Hardware-backed: no se puede extraer ni copiar. Es el límite duro del \
                    backup de credenciales."
                .into(),
            salvage: "el texto cifrado y su inventario (qué alias existen), nunca la clave."
                .into(),
        },
        NotRecoverable {
            what: "Tokens PKCS#11 y claves privadas de firma".into(),
            reason: "Por definición no salen del dispositivo: es la línea de la zona privada."
                .into(),
            salvage: "sólo el inventario (alias, huella, uso) y los hashes de la zona \
                      pública."
                .into(),
        },
    ]
}

/// Especificación de un nivel: qué incluye y qué excluye.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelSpec {
    /// El nivel.
    pub level: Level,
    /// Zona por defecto de este nivel.
    pub zone: Zone,
    /// Qué incluye.
    pub includes: Vec<String>,
    /// Qué excluye siempre.
    pub excludes: Vec<String>,
}

/// Las especificaciones de los cuatro niveles.
pub fn level_specs() -> Vec<LevelSpec> {
    vec![
        LevelSpec {
            level: Level::L0Volatile,
            zone: Zone::Public,
            includes: vec![
                "state/ (tablón, journal, sync, audit)".into(),
                "board.sqlite".into(),
                "config de la UI y preferencias".into(),
            ],
            excludes: vec![
                "los secretos: viven en md-secrets/ (0600) o dentro de la bóveda \
                 cifrada, nunca en claro en el pack"
                    .into(),
            ],
        },
        LevelSpec {
            level: Level::L1Rom,
            zone: Zone::Public,
            includes: vec![
                "boot.img, init_boot de fábrica (pack de 35,72 GB del nodo RE)".into(),
                "particiones de sólo lectura (system, vendor, product, odm)".into(),
                "las 7 particiones del mundo seguro: sec_efs, keystorage, keyrefuge, uh, \
                 uhcfg, spu, prism"
                    .into(),
            ],
            excludes: vec![
                "init_boot, vbmeta_vendor y sec: NO existen en este firmware".into(),
            ],
        },
        LevelSpec {
            level: Level::L2UserData,
            zone: Zone::Public,
            includes: vec![
                "DCIM, Downloads, Documents y /sdcard".into(),
                "datos recuperables de apps (WhatsApp Media, Discord leveldb, Telegram \
                 adjuntos, export de SMS)"
                    .into(),
            ],
            excludes: vec!["lo que AndroidKeyStore impide (ver not_recoverable())".into()],
        },
        LevelSpec {
            level: Level::L3Credentials,
            zone: Zone::Private,
            includes: vec![
                "tokens de sesión, material PKCS#11, claves de firma".into(),
            ],
            excludes: vec![
                "viaje en claro: a Drive o al repo, nunca".into(),
                "la réplica a otras instancias: la zona privada no se replica".into(),
            ],
        },
    ]
}

/// Palabras que delatan material de clave (siempre zona privada).
const PRIVATE_HINTS: [&str; 16] = [
    "private",
    "privkey",
    "priv_key",
    "secret",
    "nsec",
    "seed",
    "mnemonic",
    "keystore2",
    "pkcs11",
    "token",
    ".key",
    ".p12",
    ".pfx",
    ".jks",
    "auth_key",
    "master_key",
];

/// Palabras que delatan material público (certificados, CMS, PDF firmados).
const PUBLIC_HINTS: [&str; 10] = [
    "cert",
    ".cer",
    ".crt",
    ".pem",
    "cms",
    "signed",
    "firma",
    "public",
    "chain",
    ".pdf",
];

/// Decide la zona de una ruta.
///
/// **El orden importa**: primero lo privado (por si acaso), después el
/// prefijo `public/` y por último las pistas públicas. Ante la duda, lo
/// sensible va a `private`.
pub fn classify_zone(rel_path: &str) -> Zone {
    let p = rel_path.to_ascii_lowercase();
    if PRIVATE_HINTS.iter().any(|h| p.contains(h)) {
        return Zone::Private;
    }
    if p.starts_with("public/") {
        return Zone::Public;
    }
    if PUBLIC_HINTS.iter().any(|h| p.contains(h)) {
        return Zone::Public;
    }
    Zone::Public
}

/// Un fichero del pack con su hash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackItem {
    /// Ruta dentro del pack.
    pub rel_path: String,
    /// Nivel del que viene.
    pub level: Level,
    /// Zona.
    pub zone: Zone,
    /// Tamaño en bytes.
    pub size_bytes: u64,
    /// SHA-256 del fichero completo.
    pub sha256: String,
}

/// Un plan de pack: qué se sube, en qué nivel y de qué zona.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackPlan {
    /// Nivel del pack.
    pub level: Level,
    /// Los ficheros incluidos (los privados ya están filtrados).
    pub items: Vec<PackItem>,
    /// Bytes totales.
    pub total_bytes: u64,
    /// Los privados que se han **excluido** (constante: nunca se suben).
    pub excluded_private: Vec<String>,
}

impl PackPlan {
    /// `true` si el plan no tiene nada que subir.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Construye el plan de pack de un nivel.
///
/// **Regla dura**: los ficheros de zona privada **no entran** en el plan; se
/// listan en `excluded_private` para que quede constancia. El nivel L3 es
/// privado entero: su plan sirve para cifrar, no para subir en claro.
pub fn build_pack(level: Level, items: Vec<PackItem>) -> PackPlan {
    let mut included: Vec<PackItem> = Vec::new();
    let mut excluded: Vec<String> = Vec::new();
    for mut it in items {
        // En L3 todo es privado por definición.
        if level == Level::L3Credentials {
            it.zone = Zone::Private;
        }
        if it.zone == Zone::Private {
            excluded.push(it.rel_path.clone());
            continue;
        }
        included.push(it);
    }
    let total_bytes = included.iter().map(|i| i.size_bytes).sum();
    PackPlan {
        level,
        items: included,
        total_bytes,
        excluded_private: excluded,
    }
}

/// Un trozo de un fichero, con su hash: la unidad de subida a Drive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkRef {
    /// Ruta del fichero.
    pub rel_path: String,
    /// Índice del trozo (0 = primero).
    pub index: u32,
    /// Desplazamiento dentro del fichero.
    pub offset: u64,
    /// Longitud del trozo.
    pub len: u64,
    /// SHA-256 del trozo.
    pub sha256: String,
}

/// Calcula el SHA-256 de un trozo del fichero sin cargarlo entero en memoria.
pub fn chunk_sha256(path: &Path, offset: u64, len: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = fs::File::open(path).ok()?;
    f.seek(SeekFrom::Start(offset)).ok()?;
    let mut hasher = Sha256::new();
    let mut remaining = len;
    let mut buf = vec![0u8; 64 * 1024];
    while remaining > 0 {
        let want = std::cmp::min(remaining as usize, buf.len());
        let n = f.read(&mut buf[..want]).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        remaining -= n as u64;
    }
    Some(sha256_hex(&hasher.finalize()))
}

/// Divide el pack en trozos para subirlos **por streaming** a Drive.
///
/// Cada trozo lleva su SHA-256 y su desplazamiento: no hace falta ninguna
/// copia local intermedia (el móvil tiene 5,9 GB libres y el pack son
/// decenas de GB). `source_dir` es el directorio donde viven los ficheros; si
/// un fichero no está, el trozo se marca con hash vacío en vez de inventar uno.
pub fn plan_chunks(plan: &PackPlan, source_dir: &Path, chunk_size: u64) -> Vec<ChunkRef> {
    let mut out: Vec<ChunkRef> = Vec::new();
    if chunk_size == 0 {
        return out;
    }
    for item in &plan.items {
        let full = source_dir.join(&item.rel_path);
        let mut offset: u64 = 0;
        let mut index: u32 = 0;
        loop {
            let len = std::cmp::min(chunk_size, item.size_bytes.saturating_sub(offset));
            if len == 0 {
                break;
            }
            let sha = chunk_sha256(&full, offset, len).unwrap_or_default();
            out.push(ChunkRef {
                rel_path: item.rel_path.clone(),
                index,
                offset,
                len,
                sha256: sha,
            });
            offset += len;
            index += 1;
            if offset >= item.size_bytes {
                break;
            }
        }
    }
    out
}

/// Resultado de verificar el pack en el destino.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestinationReport {
    /// Cuántos ficheros casaron.
    pub verified: usize,
    /// Los que no casaron: (ruta, esperado, obtenido).
    pub mismatched: Vec<(String, String, String)>,
    /// Los que no se pudieron comprobar (no estaban en el destino).
    pub missing: Vec<String>,
}

impl DestinationReport {
    /// `true` si todo casó.
    pub fn ok(&self) -> bool {
        self.mismatched.is_empty() && self.missing.is_empty()
    }
}

/// Verifica el pack **en el destino** (otra instancia, otro disco, Drive).
///
/// `lookup` recibe la ruta relativa y devuelve el SHA-256 que tiene el
/// destino (o `None` si no está). Es la comprobación que hace falta para
/// poder decir que la zona pública está replicada de verdad.
pub fn verify_at_destination(
    plan: &PackPlan,
    lookup: impl Fn(&str) -> Option<String>,
) -> DestinationReport {
    let mut verified = 0usize;
    let mut mismatched: Vec<(String, String, String)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for it in &plan.items {
        match lookup(&it.rel_path) {
            None => missing.push(it.rel_path.clone()),
            Some(h) if h.eq_ignore_ascii_case(&it.sha256) => verified += 1,
            Some(h) => mismatched.push((it.rel_path.clone(), it.sha256.clone(), h)),
        }
    }
    DestinationReport {
        verified,
        mismatched,
        missing,
    }
}

/// Resumen por nivel y por zona (para el informe del backup).
pub fn level_report(plan: &PackPlan) -> serde_json::Value {
    use serde_json::json;
    let public: usize = plan.items.iter().filter(|i| i.zone == Zone::Public).count();
    let private_excluded = plan.excluded_private.len();
    json!({
        "level": plan.level.as_str(),
        "title": plan.level.title(),
        "files": plan.items.len(),
        "bytes": plan.total_bytes,
        "public_files": public,
        "private_excluded": private_excluded,
        "replicable": plan.level != Level::L3Credentials,
        "excluded_private": plan.excluded_private,
    })
}

// ---------------------------------------------------------------------------
// W3 · Navegadores: inventario real y estrategia de copia y sincronización
// ---------------------------------------------------------------------------

/// Motor de un navegador: decide dónde están los datos y qué se puede llevar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserEngine {
    /// Chromium (Chrome, Thorium, Edge, Brave…). Perfil en `app_chrome/Default`.
    Chromium,
    /// Gecko (Firefox, Fenix, Iceraven, Fennec, Tor). Perfil en `files/mozilla`.
    Gecko,
    /// Samsung Internet. Perfil en `app_sbrowser/Default`.
    SamsungInternet,
    /// App con WebView pero sin perfil de navegador propio.
    WebViewOnly,
}

impl BrowserEngine {
    /// Nombre estable.
    pub fn as_str(&self) -> &'static str {
        match self {
            BrowserEngine::Chromium => "chromium",
            BrowserEngine::Gecko => "gecko",
            BrowserEngine::SamsungInternet => "samsung",
            BrowserEngine::WebViewOnly => "webview",
        }
    }

    /// Rutas dentro de `/data/user/0/<pkg>` donde vive el perfil del motor.
    pub fn profile_paths(&self) -> &'static [&'static str] {
        match self {
            BrowserEngine::Chromium => &[
                "app_chrome/Default/Bookmarks",
                "app_chrome/Default/History",
                "app_chrome/Default/Preferences",
                "app_chrome/Default/Web Data",
                "app_chrome/Local State",
            ],
            BrowserEngine::Gecko => &[
                "files/mozilla/profiles.ini",
                "files/mozilla/<perfil>.default/places.sqlite",
                "files/mozilla/<perfil>.default/cookies.sqlite",
                "files/mozilla/<perfil>.default/formhistory.sqlite",
                "files/mozilla/<perfil>.default/content-prefs.sqlite",
                "files/mozilla/<perfil>.default/cert9.db",
            ],
            BrowserEngine::SamsungInternet => &[
                "app_sbrowser/Default/Bookmarks",
                "app_sbrowser/Default/History",
                "app_sbrowser/Default/Preferences",
                "app_sbrowser/Local State",
            ],
            BrowserEngine::WebViewOnly => &["app_webview/Default/Preferences"],
        }
    }
}

/// Qué se puede llevar a otra instancia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Portability {
    /// Marcadores e historial: SQLite/JSON sin cifrar, portables.
    BookmarksHistory,
    /// Perfil casi entero, pero las contraseñas están atadas al keystore.
    ProfileWithBoundSecrets,
    /// Sólo caché y datos regenerables: **no merece la pena respaldarlo**.
    CacheOnly,
}

impl Portability {
    /// Nombre estable.
    pub fn as_str(&self) -> &'static str {
        match self {
            Portability::BookmarksHistory => "bookmarks_history",
            Portability::ProfileWithBoundSecrets => "profile_with_bound_secrets",
            Portability::CacheOnly => "cache_only",
        }
    }
}

/// Un navegador instalado y su estrategia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserProfile {
    /// Paquete Android.
    pub package: String,
    /// Nombre legible.
    pub label: String,
    /// Motor.
    pub engine: BrowserEngine,
    /// Sincronización nativa disponible (la mejor vía para replicar).
    pub native_sync: Option<String>,
    /// Formato de exportación de marcadores.
    pub export: Option<String>,
    /// Qué se puede llevar a otra instancia.
    pub portability: Portability,
    /// Rutas que están atadas al Android Keystore (no portables).
    pub keystore_bound: Vec<String>,
    /// Nota honesta sobre este navegador en este dispositivo.
    pub note: String,
}

/// Los navegadores **reales** de este dispositivo, medidos.
///
/// Inventario del S21 Ultra (Samsung SM-G998B, Android 14) obtenido del
/// resolver table de `dumpsys package` y de las firmas de perfil en
/// `/data/user/0/<pkg>`: los que tienen `app_chrome/Default/Bookmarks`
/// (Chromium), `files/mozilla/` (Gecko) o `app_sbrowser/` (Samsung).
///
/// Los ~500 `app_webview/` que hay en el móvil **no** son navegadores: son
/// el perfil WebView de apps que abren enlaces. Su masa es casi todo caché
/// regenerable y no entra en el plan.
pub fn browser_profiles() -> Vec<BrowserProfile> {
    let chromium_bound = vec![
        "app_chrome/Local State (os_crypt.encrypted_key)".to_string(),
        "app_chrome/Default/Login Data (valores cifrados con la clave del keystore)".to_string(),
        "Cookies (cifrados con la clave del keystore)".to_string(),
    ];
    let gecko_bound = vec![
        "files/mozilla/<perfil>.default/key4.db (clave maestra NSS atada al keystore)".to_string(),
        "files/mozilla/<perfil>.default/logins.json (cifrado NSS)".to_string(),
    ];
    vec![
        BrowserProfile {
            package: "com.android.chrome".into(),
            label: "Chrome".into(),
            engine: BrowserEngine::Chromium,
            native_sync: Some("Google Sync (cuenta de Google)".into()),
            export: Some("HTML".into()),
            portability: Portability::ProfileWithBoundSecrets,
            keystore_bound: chromium_bound.clone(),
            note: "1,2 GB. Los marcadores y el historial son SQLite/JSON planos y \
                   portables; las cookies y las contraseñas están cifradas con la \
                   clave del keystore y no sobreviven a otro dispositivo. La vía \
                   que sí replica es Google Sync."
                .into(),
        },
        BrowserProfile {
            package: "com.chrome.dev".into(),
            label: "Chrome Dev".into(),
            engine: BrowserEngine::Chromium,
            native_sync: Some("Google Sync (perfil de desarrollador)".into()),
            export: Some("HTML".into()),
            portability: Portability::ProfileWithBoundSecrets,
            keystore_bound: chromium_bound.clone(),
            note: "Compilación para desarrolladores, 160 MB.".into(),
        },
        BrowserProfile {
            package: "org.chromium.thorium".into(),
            label: "Thorium".into(),
            engine: BrowserEngine::Chromium,
            native_sync: None,
            export: Some("HTML".into()),
            portability: Portability::ProfileWithBoundSecrets,
            keystore_bound: chromium_bound.clone(),
            note: "274 MB. Sin sincronización nativa: se replica con copia root \
                   del perfil + exportación de marcadores a HTML."
                .into(),
        },
        BrowserProfile {
            package: "com.microsoft.emmx".into(),
            label: "Edge".into(),
            engine: BrowserEngine::Chromium,
            native_sync: Some("Microsoft Edge Sync".into()),
            export: Some("HTML".into()),
            portability: Portability::ProfileWithBoundSecrets,
            keystore_bound: chromium_bound.clone(),
            note: "95 MB. Chromium con su propio perfil en app_chrome/.".into(),
        },
        BrowserProfile {
            package: "org.mozilla.fenix".into(),
            label: "Firefox".into(),
            engine: BrowserEngine::Gecko,
            native_sync: Some("Firefox Sync (cuenta Firefox, cifrado de extremo a \
                               extremo)".into()),
            export: Some("HTML / JSON".into()),
            portability: Portability::ProfileWithBoundSecrets,
            keystore_bound: gecko_bound.clone(),
            note: "1,1 GB, el mayor perfil Gecko. **La mejor vía de réplica**: \
                   Firefox Sync cifra de extremo a extremo con la contraseña de la \
                   cuenta, que no es la del keystore, así que los marcadores, \
                   contraseñas e historial SÍ son portables entre dispositivos."
                .into(),
        },
        BrowserProfile {
            package: "io.github.forkmaintainers.iceraven".into(),
            label: "Iceraven".into(),
            engine: BrowserEngine::Gecko,
            native_sync: Some("Firefox Sync (fork de Fenix: comprobar)".into()),
            export: Some("HTML / JSON".into()),
            portability: Portability::ProfileWithBoundSecrets,
            keystore_bound: gecko_bound.clone(),
            note: "2,3 GB: **el mayor bloque de datos de navegador del móvil**. \
                   Fork de Fenix: el perfil es Gecko estándar, así que la copia \
                   root del perfil sirve tal cual."
                .into(),
        },
        BrowserProfile {
            package: "us.spotco.fennec_dos".into(),
            label: "Fennec".into(),
            engine: BrowserEngine::Gecko,
            native_sync: Some("Firefox Sync".into()),
            export: Some("HTML / JSON".into()),
            portability: Portability::ProfileWithBoundSecrets,
            keystore_bound: gecko_bound.clone(),
            note: "24 MB, perfil Gecko clásico.".into(),
        },
        BrowserProfile {
            package: "org.torproject.torbrowser".into(),
            label: "Tor Browser".into(),
            engine: BrowserEngine::Gecko,
            native_sync: None,
            export: Some("HTML".into()),
            portability: Portability::ProfileWithBoundSecrets,
            keystore_bound: gecko_bound.clone(),
            note: "58 MB. **Sin sincronización por diseño** (el estado de Tor es \
                   efímero a propósito): sólo exportación de marcadores a HTML."
                .into(),
        },
        BrowserProfile {
            package: "org.torproject.torbrowser_alpha".into(),
            label: "Tor Browser Alpha".into(),
            engine: BrowserEngine::Gecko,
            native_sync: None,
            export: Some("HTML".into()),
            portability: Portability::ProfileWithBoundSecrets,
            keystore_bound: gecko_bound.clone(),
            note: "88 MB. Canal alpha de Tor, mismo criterio.".into(),
        },
        BrowserProfile {
            package: "com.sec.android.app.sbrowser".into(),
            label: "Samsung Internet".into(),
            engine: BrowserEngine::SamsungInternet,
            native_sync: Some("Samsung Pass / cuenta Samsung".into()),
            export: Some("HTML".into()),
            portability: Portability::ProfileWithBoundSecrets,
            keystore_bound: vec![
                "app_sbrowser/Local State".to_string(),
                "app_sbrowser/Default/Cookies".to_string(),
            ],
            note: "545 MB. El navegador por defecto del móvil.".into(),
        },
        BrowserProfile {
            package: "com.sec.android.app.sbrowser.beta".into(),
            label: "Samsung Internet Beta".into(),
            engine: BrowserEngine::SamsungInternet,
            native_sync: Some("Samsung Pass / cuenta Samsung".into()),
            export: Some("HTML".into()),
            portability: Portability::ProfileWithBoundSecrets,
            keystore_bound: vec![
                "app_sbrowser/Local State".to_string(),
                "app_sbrowser/Default/Cookies".to_string(),
            ],
            note: "214 MB. Canal beta: el perfil es independiente del estable.".into(),
        },
        BrowserProfile {
            package: "com.fixupxer".into(),
            label: "FixupXer".into(),
            engine: BrowserEngine::WebViewOnly,
            native_sync: None,
            export: None,
            portability: Portability::CacheOnly,
            keystore_bound: vec!["app_webview/Default/Cookies".to_string()],
            note: "533 KB. Navegador sobre WebView con bloqueador de anuncios: su \
                   sesión vive en el perfil WebView."
                .into(),
        },
        BrowserProfile {
            package: "com.myAllVideoBrowser".into(),
            label: "MyAllVideoBrowser".into(),
            engine: BrowserEngine::WebViewOnly,
            native_sync: None,
            export: None,
            portability: Portability::CacheOnly,
            keystore_bound: vec!["app_webview/Default/Cookies".to_string()],
            note: "144 MB. App WebView sin sincronización.".into(),
        },
    ]
}

/// Rutas que **no** hay que respaldar: son caché regenerable.
///
/// Con 5,9 GB libres y más de 20 GB de datos de navegador, copiar la caché es
/// la diferencia entre un backup que cabe y uno que no.
pub fn browser_exclude_paths() -> Vec<&'static str> {
    vec![
        "cache",
        "code_cache",
        "GPUCache",
        "DawnCache",
        "DawnGraphiteCache",
        "DawnWebGPUCache",
        "Service Worker/CacheStorage",
        "Service Worker/ScriptCache",
        "blob_storage",
        "files/mozilla/<perfil>.default/cache2",
        "files/mozilla/<perfil>.default/startupCache",
        "media",
        "Shared Memory",
        "app_webview/Default/Cache",
        "app_webview/Default/Code Cache",
        "app_webview/Default/GPUCache",
    ]
}

/// Estrategia de copia y sincronización por navegador.
///
/// Prioriza la vía **nativa** (que replica de verdad entre instancias) y sólo
/// cae en la copia root cuando no la hay.
pub fn browser_sync_strategy(profile: &BrowserProfile) -> Vec<String> {
    let mut steps: Vec<String> = Vec::new();
    match profile.native_sync {
        Some(sync) => {
            steps.push(format!(
                "1º Sincronización nativa ({}): es la única vía que replica \
                 contraseñas, que están atadas al keystore de este dispositivo.",
                sync
            ));
        }
        None => steps.push(
            "1º No hay sincronización nativa: la réplica tiene que hacerse con \
             copia root del perfil (root ya está en este móvil)."
                .to_string(),
        ),
    }
    steps.push(format!(
        "2º Copia root del perfil ({}) con SHA-256 por fichero: sólo lo portable \
         (marcadores, historial, preferencias), nunca la caché.",
        profile.engine.as_str()
    ));
    if let Some(export) = &profile.export {
        steps.push(format!(
            "3º Export de marcadores a {} como red de seguridad: es portable \
             entre cualquier navegador.",
            export
        ));
    } else {
        steps.push(
            "3º Sin exportación: el perfil WebView no expone marcadores.".to_string()
        );
    }
    steps.push(
        "4º En destino: se comprueba el SHA-256 de cada fichero antes de dar \
         el backup por bueno (sin copia local intermedia)."
            .to_string(),
    );
    if !profile.keystore_bound.is_empty() {
        steps.push(format!(
            "5º NO portable (atado al keystore): {} — se documenta, no se copia.",
            profile.keystore_bound.join(", ")
        ));
    }
    steps
}

/// Los navegadores que **no** se deben respaldar (sólo caché) y el motivo.
pub fn browser_backup_exclusions() -> Vec<(String, String)> {
    vec![
        (
            "los ~500 perfiles app_webview/ del móvil".into(),
            "perfil WebView compartido de apps que abren enlaces: casi todo es \
             caché regenerable. Respalden lo que importa (ficheros del usuario), \
             no el perfil WebView completo"
                .into(),
        ),
        (
            "cache/, code_cache/, GPUCache/, Service Worker/CacheStorage".into(),
            "regenerable: son bytes que la app vuelve a descargar".into(),
        ),
        (
            "cookies y contraseñas cifradas con clave del keystore".into(),
            "no son portables: el keystore no sale del dispositivo. Se copian \
             dentro de la zona privada cifrada, o no se copian"
                .into(),
        ),
    ]
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

    // --- W3 · niveles, zonas y plan de pack ---------------------------------

    fn item(rel: &str, bytes: u64) -> PackItem {
        PackItem {
            rel_path: rel.to_string(),
            level: Level::L2UserData,
            zone: classify_zone(rel),
            size_bytes: bytes,
            sha256: sha256_hex(rel.as_bytes()),
        }
    }

    #[test]
    fn la_zona_privada_manda_sobre_la_publica() {
        // Una clave dentro de una ruta "public/" sigue siendo privada.
        assert_eq!(classify_zone("public/privkey.pem"), Zone::Private);
        assert_eq!(classify_zone("keys/pkcs11.token"), Zone::Private);
        assert_eq!(classify_zone("public/cert-chain.pem"), Zone::Public);
        assert_eq!(classify_zone("public/firma.pdf"), Zone::Public);
        assert_eq!(classify_zone("dcim/IMG_0001.jpg"), Zone::Public);
    }

    #[test]
    fn el_plan_excluye_lo_privado_y_lo_deja_constancia() {
        let plan = build_pack(
            Level::L2UserData,
            vec![
                item("dcim/foto.jpg", 100),
                item("keys/maestro.key", 10),
                item("public/cert.pem", 20),
            ],
        );
        assert_eq!(plan.items.len(), 2);
        assert!(plan.items.iter().all(|i| i.zone == Zone::Public));
        assert_eq!(plan.excluded_private, vec!["keys/maestro.key".to_string()]);
        assert_eq!(plan.total_bytes, 120);
    }

    #[test]
    fn el_nivel_l3_es_privado_entero() {
        let plan = build_pack(
            Level::L3Credentials,
            vec![item("token.json", 5), item("public/cert.pem", 5)],
        );
        assert!(plan.items.is_empty());
        assert_eq!(plan.excluded_private.len(), 2);
        assert!(!level_report(&plan)["replicable"].as_bool().unwrap());
    }

    #[test]
    fn los_cuatro_niveles_declaran_incluye_y_excluye() {
        let specs = level_specs();
        assert_eq!(specs.len(), 4);
        assert_eq!(specs[0].level, Level::L0Volatile);
        assert_eq!(specs[3].level, Level::L3Credentials);
        assert_eq!(specs[3].zone, Zone::Private);
        // L1 tiene que declarar las 7 particiones del mundo seguro y que
        // las del volcado no existen.
        let l1 = specs.iter().find(|s| s.level == Level::L1Rom).unwrap();
        let txt = l1.includes.join(" ");
        for p in EFS_SAFE_WORLD.iter() {
            assert!(txt.contains(p), "falta {}", p);
        }
        assert!(l1.excludes.join(" ").contains("vbmeta_vendor"));
    }

    #[test]
    fn las_particiones_inexistentes_no_se_inventan() {
        for p in EFS_NON_EXISTENT.iter() {
            assert!(
                !EFS_SAFE_WORLD.contains(p),
                "{} no existe en este firmware",
                p
            );
        }
        assert_eq!(EFS_SAFE_WORLD.len(), 7);
    }

    #[test]
    fn lo_no_recuperable_se_declara_con_motivo_y_rescate() {
        let nr = not_recoverable();
        assert!(nr.len() >= 4);
        assert!(nr.iter().all(|n| !n.what.is_empty() && !n.reason.is_empty()));
        assert!(nr
            .iter()
            .any(|n| n.what.contains("keystore2") || n.what.contains("Keystore")));
        // Lo irrecuperable debe explicar qué se salva.
        assert!(nr.iter().all(|n| !n.salvage.is_empty()));
    }

    #[test]
    fn el_plan_de_chunks_divide_sin_copia_local() {
        let dir = tmpdir("chunks");
        fs::write(format!("{}/datos.bin", dir), vec![7u8; 2500]).unwrap();
        let plan = build_pack(
            Level::L2UserData,
            vec![PackItem {
                rel_path: "datos.bin".into(),
                level: Level::L2UserData,
                zone: Zone::Public,
                size_bytes: 2500,
                sha256: sha256_file(Path::new(&format!("{}/datos.bin", dir))).unwrap(),
            }],
        );
        let chunks = plan_chunks(&plan, Path::new(&dir), 1000);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].offset, 0);
        assert_eq!(chunks[0].len, 1000);
        assert_eq!(chunks[2].len, 500);
        // Cada trozo lleva su hash y son distintos.
        assert_eq!(chunks[0].sha256.len(), 64);
        assert_ne!(chunks[0].sha256, chunks[1].sha256);
        // El último trozo debe coincidir con el final del fichero.
        assert_eq!(
            chunk_sha256(Path::new(&format!("{}/datos.bin", dir)), 2000, 500).unwrap(),
            chunks[2].sha256
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verificar_en_el_destino_detecta_que_no_casa() {
        let plan = build_pack(
            Level::L2UserData,
            vec![item("a.txt", 1), item("b.txt", 1), item("c.txt", 1)],
        );
        let mut lookup = |rel: &str| -> Option<String> {
            match rel {
                "a.txt" => Some(plan.items[0].sha256.clone()),
                "b.txt" => Some("0".repeat(64)),
                _ => None,
            }
        };
        let rep = verify_at_destination(&plan, &mut lookup);
        assert_eq!(rep.verified, 1);
        assert_eq!(rep.mismatched.len(), 1);
        assert_eq!(rep.missing, vec!["c.txt".to_string()]);
        assert!(!rep.ok());
    }

    // --- W3 · navegadores ----------------------------------------------------

    #[test]
    fn el_inventario_de_navegadores_cubre_los_cuatro_motores() {
        let b = browser_profiles();
        assert!(b.len() >= 12, "el móvil tiene más de 12 navegadores reales");
        let motores: Vec<&str> = b.iter().map(|x| x.engine.as_str()).collect();
        assert!(motores.contains(&"chromium"));
        assert!(motores.contains(&"gecko"));
        assert!(motores.contains(&"samsung"));
        assert!(motores.contains(&"webview"));
        // Los que el usuario tiene de verdad.
        for p in [
            "com.android.chrome",
            "org.chromium.thorium",
            "org.mozilla.fenix",
            "io.github.forkmaintainers.iceraven",
            "org.torproject.torbrowser",
            "com.sec.android.app.sbrowser",
        ] {
            assert!(
                b.iter().any(|x| x.package == p),
                "falta el navegador {}",
                p
            );
        }
    }

    #[test]
    fn la_sincronizacion_nativa_tiene_prioridad_sobre_la_copia_root() {
        let b = browser_profiles();
        let firefox = b.iter().find(|x| x.package == "org.mozilla.fenix").unwrap();
        let pasos = browser_sync_strategy(firefox);
        assert!(pasos.len() >= 4);
        assert!(pasos[0].contains("Firefox Sync"));

        // Thorium y Tor no la tienen: la estrategia dice que hay que copiar.
        for pkg in ["org.chromium.thorium", "org.torproject.torbrowser"] {
            let p = b.iter().find(|x| x.package == pkg).unwrap();
            let pasos = browser_sync_strategy(p);
            assert!(
                pasos[0].contains("copia root"),
                "{} debería indicar la copia root: {:?}",
                pkg,
                pasos[0]
            );
        }
    }

    #[test]
    fn la_cache_y_lo_atado_al_keystore_quedan_fuera_del_plan() {
        let exc = browser_exclude_paths();
        assert!(exc.iter().any(|p| p.contains("cache")));
        assert!(exc.iter().any(|p| p.contains("GPUCache")));
        assert!(exc.iter().any(|p| p.contains("CacheStorage")));

        let exclusiones = browser_backup_exclusions();
        assert!(exclusiones
            .iter()
            .any(|(q, _)| q.contains("app_webview")));
        assert!(exclusiones
            .iter()
            .any(|(_, r)| r.contains("keystore")));
    }

    #[test]
    fn el_perfil_de_cada_motor_apunta_a_sus_rutas_reales() {
        let chromium = BrowserEngine::Chromium.profile_paths();
        assert!(chromium.contains(&"app_chrome/Default/Bookmarks"));
        let gecko = BrowserEngine::Gecko.profile_paths();
        assert!(gecko.iter().any(|p| p.contains("places.sqlite")));
        let samsung = BrowserEngine::SamsungInternet.profile_paths();
        assert!(samsung.iter().any(|p| p.contains("app_sbrowser")));
    }
}
