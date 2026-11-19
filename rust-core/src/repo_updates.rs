//! MegaDaemon v13.4 — Índice de repositorios y actualización automática de apps.
//!
//! MegaDaemon mantiene las apps al día sin que el usuario tenga que abrir
//! F-Droid, Obtainium o la Play Store.  Este módulo es el "cerebro" de esa
//! cadena y está pensado para vivir en un MÓVIL con root (Android) y también
//! en un PC Linux:
//!
//! ```text
//!   repos()       → catálogo de fuentes (F-Droid, Obtainium, Play, genéricos)
//!   fetch_index() → baja el índice JSON (ureq), lo NORMALIZA y lo cachea
//!   plan()        → cruza índices + inventario local → qué se actualiza y con
//!                   qué riesgo (low/medium/high); qué queda BLOQUEADO y por qué
//!   download()    → baja el APK y VERIFICA el SHA-256 (si no cuadra, borra)
//!   apply()       → `pm install -r -g` como root — EXIGE confirm == true
//!   watch_once()  → ciclo NO destructivo: fetch + plan (nada de descargas)
//!   status()      → repos, edad del último índice, caché, apps vigiladas
//! ```
//!
//! ── REGLAS DE ORO (v13.4) ───────────────────────────────────────────────────
//!
//!   * **Nunca entra en panic.**  Todo el parseo es tolerante: si falta un
//!     campo se ignora, si el JSON está roto se devuelve `ok:false` con el
//!     motivo, y una entrada sin `package` se descarta en silencio.
//!   * **Nunca se queda colgado.**  Los timeouts HTTP son cortos (8 s global,
//!     4 s de conexión) y, si la red falla, el sistema **cae a la caché** en
//!     disco y marca la respuesta con `stale:true`.
//!   * **Nunca instala nada sin `confirm == true`.**  `apply()` sin
//!     confirmación es un no-op explícito.
//!
//! ── FORMATO NORMALIZADO DE UNA ENTRADA ──────────────────────────────────────
//!
//! Todos los repos se reducen al mismo esquema, para que el cruce con el
//! inventario local sea trivial:
//!
//! ```json
//! { "package": "org.fdroid.fdroid", "version_name": "0.108.0",
//!   "version_code": 10800, "apk_url": "https://…/f.droid_10800.apk",
//!   "sha256": "9f2c…", "size": 8123456, "added_date": "2024-05-02 10:00:00Z" }
//! ```
//!
//! ── SOBRE GOOGLE PLAY ───────────────────────────────────────────────────────
//!
//! Play **no** tiene API pública: el catálogo de versiones y la descarga de
//! APK sólo existen dentro de la app oficial y para la cuenta del usuario.
//! No hay ningún endpoint honesto que consultar, así que aquí NO se inventa
//! ninguno: el repo `play` se declara explícitamente como "requiere
//! cuenta / instalador externo" y el plan nunca lo usa como fuente de
//! actualizaciones.
//!
//! ── CONFIGURACIÓN ───────────────────────────────────────────────────────────
//!
//! El usuario puede añadir sus propios repositorios en
//! `<state_dir>/repos.json` (mismo formato que el catálogo):
//!
//! ```json
//! [ { "id": "obtainium-casa", "name": "Obtainium (casa)",
//!     "kind": "obtainium", "index_url": "https://host/index.json",
//!     "auth": "token", "token": "…", "direct_download": true } ]
//! ```
//!
//! Además, `MEGADAEMON_OBTAINIUM_INDEX_URL` apunta al índice Obtainium local
//! sin necesidad de tocar ningún fichero.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── Constantes ───────────────────────────────────────────────────────────────

/// User-Agent que identifica al daemon ante F-Droid/Obtainium.
pub const USER_AGENT: &str = "MegaDaemon/13.4 (+https://github.com/boolforge/megadaemon)";

/// Timeout global de las peticiones de índice (segundos).
pub const INDEX_TIMEOUT_SECS: u64 = 8;
/// Timeout de conexión (segundos).
pub const CONNECT_TIMEOUT_SECS: u64 = 4;
/// Timeout de la descarga de un APK (segundos): son ficheros grandes.
pub const APK_TIMEOUT_SECS: u64 = 300;

/// Tope de bytes de un índice descargado (64 MiB): los índices de F-Droid
/// pesan varios MiB, pero no hay razón para bajar cientos.
pub const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;
/// Tope de bytes de un APK (512 MiB).
pub const MAX_APK_BYTES: u64 = 512 * 1024 * 1024;

/// Antigüedad máxima de una caché para considerarse "fresca" (6 horas).
pub const CACHE_TTL_SECS: u64 = 6 * 3600;

/// Número máximo de apps inventariadas de una tacada.
pub const DEFAULT_APP_LIMIT: usize = 5000;

/// Timeout de los comandos `pm`/`dumpsys` del inventario (ms).
const PM_TIMEOUT_MS: u64 = 15_000;
/// Tope de caracteres de la salida de `dumpsys` (el de un móvil con 400 apps
/// ronda los 300 KB).
const PM_MAX_CHARS: usize = 512 * 1024;

// ── Especificación de un repositorio ───────────────────────────────────────

/// Descripción de una fuente de apps.
///
/// `index_urls` son URLs *candidatas*: se prueban en orden hasta que una
/// responde, de modo que un mirror caído no deja al daemon sin índice.
#[derive(Clone, Debug)]
pub struct RepoSpec {
    /// Identificador estable (se usa como nombre de fichero de la caché).
    pub id: String,
    /// Nombre legible para la UI.
    pub name: String,
    /// Tipo de repositorio: `fdroid`, `obtainium`, `play` o `generic`.
    pub kind: String,
    /// URLs candidatas del índice (vacío = fuente no utilizable todavía).
    pub index_urls: Vec<String>,
    /// Autenticación: `none`, `cookie` o `token`.
    pub auth: String,
    /// Credencial para `auth: "token"` (cabecera `Authorization: Bearer …`).
    pub token: String,
    /// Credencial para `auth: "cookie"` (cabecera `Cookie: …`).
    pub cookie: String,
    /// ¿Se puede descargar el APK directamente desde la URL del índice?
    pub direct_download: bool,
    /// ¿La fuente está lista para usarse (tiene URL de índice)?
    pub configured: bool,
    /// Explicación para la UI: por qué no se puede usar, o de dónde sale.
    pub note: String,
}

impl RepoSpec {
    /// Ficha JSON de la fuente (tal y como la ve la UI).
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "kind": self.kind,
            "type": self.kind,              // alias: la UI histórica usa "type"
            "index_url": self.index_urls.first().cloned().unwrap_or_default(),
            "index_urls": self.index_urls,
            "auth": self.auth,
            "direct_download": self.direct_download,
            "configured": self.configured,
            "note": self.note,
        })
    }
}

/// Catálogo de fuentes por defecto de MegaDaemon.
///
/// F-Droid va primero porque es el único con hash SHA-256 por APK (permite
/// verificar la integridad antes de instalar).  Obtainium entra con la URL
/// que el usuario configure.  Play se declara, pero SIN endpoint inventado.
fn builtin_repos() -> Vec<RepoSpec> {
    let obtainium_url = std::env::var("MEGADAEMON_OBTAINIUM_INDEX_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    vec![
        RepoSpec {
            id: "fdroid".into(),
            name: "F-Droid (mirror fdlf01)".into(),
            kind: "fdroid".into(),
            index_urls: vec![
                "https://fdlf01.f-droid.org/repo/index-v1.json".into(),
                "https://f-droid.org/repo/index-v1.json".into(),
            ],
            auth: "none".into(),
            token: String::new(),
            cookie: String::new(),
            direct_download: true,
            configured: true,
            note: "Índice oficial v1: cada entrada trae versionCode y hash SHA-256 del APK.".into(),
        },
        RepoSpec {
            id: "fdroid-official".into(),
            name: "F-Droid (servidor oficial)".into(),
            kind: "fdroid".into(),
            index_urls: vec!["https://f-droid.org/repo/index-v1.json".into()],
            auth: "none".into(),
            token: String::new(),
            cookie: String::new(),
            direct_download: true,
            configured: true,
            note: "Servidor principal de F-Droid (el mirror fdlf01 es la copia de lectura rápida).".into(),
        },
        RepoSpec {
            id: "obtainium".into(),
            name: "Obtainium (índice propio)".into(),
            kind: "obtainium".into(),
            index_urls: obtainium_url.clone().into_iter().collect(),
            auth: "none".into(),
            token: String::new(),
            cookie: String::new(),
            direct_download: true,
            configured: obtainium_url.is_some(),
            note: "Define MEGADAEMON_OBTAINIUM_INDEX_URL=https://<host>/index.json \
                   o añade el repo en <state_dir>/repos.json (formato: id, name, kind, index_url, auth, token)."
                .into(),
        },
        RepoSpec {
            id: "play".into(),
            name: "Google Play".into(),
            kind: "play".into(),
            index_urls: Vec::new(),
            auth: "token".into(),
            token: String::new(),
            cookie: String::new(),
            direct_download: false,
            configured: false,
            note: "Google Play NO ofrece API pública de catálogo ni de descarga de APK: \
                   la versión de cada app y el propio fichero sólo están disponibles \
                   dentro de la app oficial y para la cuenta del usuario (o con un \
                   instalador externo tipo Aurora, que necesita sus credenciales y \
                   va contra las condiciones de servicio). Por eso aquí no hay ningún \
                   endpoint que consultar: se gestiona desde la Play Store del \
                   dispositivo, o desde Obtainium con Play como fuente."
                .into(),
        },
        RepoSpec {
            id: "generic".into(),
            name: "Repositorio genérico (JSON)".into(),
            kind: "generic".into(),
            index_urls: Vec::new(),
            auth: "none".into(),
            token: String::new(),
            cookie: String::new(),
            direct_download: true,
            configured: false,
            note: "Plantilla para repositorios propios: añade la entrada en <state_dir>/repos.json \
                   con kind \"generic\" y su index_url."
                .into(),
        },
    ]
}

/// Repositorios que el usuario ha añadido en `<state_dir>/repos.json`.
///
/// Parseo totalmente tolerante: una entrada rota se ignora en lugar de
/// romper el arranque del daemon.
fn user_repos() -> Vec<RepoSpec> {
    let path = PathBuf::from(crate::cockpit_server::state_dir()).join("repos.json");
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let parsed: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let list = match &parsed {
        Value::Array(a) => a.clone(),
        Value::Object(o) => o.get("repos").and_then(|r| r.as_array()).cloned().unwrap_or_default(),
        _ => Vec::new(),
    };

    let mut out = Vec::new();
    for e in list {
        let id = match first_str(&e, &["id", "name"]) {
            Some(i) if !i.is_empty() => i,
            _ => continue,
        };
        let kind = first_str(&e, &["kind", "type"]).unwrap_or_else(|| "generic".to_string());
        let index_url = first_str(&e, &["index_url", "indexUrl", "url", "index"]).unwrap_or_default();
        let auth = first_str(&e, &["auth", "auth_type"]).unwrap_or_else(|| "none".to_string());
        let token = first_str(&e, &["token", "api_key", "apikey"]).unwrap_or_default();
        let cookie = first_str(&e, &["cookie"]).unwrap_or_default();
        let direct = e
            .get("direct_download")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let name = first_str(&e, &["name"]).unwrap_or_else(|| id.clone());
        let note = first_str(&e, &["note"]).unwrap_or_else(|| "Repo definido por el usuario.".to_string());

        let index_urls: Vec<String> = if index_url.is_empty() {
            Vec::new()
        } else {
            // Se admiten varias URLs separadas por coma (con mirrors).
            index_url
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };
        out.push(RepoSpec {
            configured: !index_urls.is_empty(),
            id,
            name,
            kind,
            index_urls,
            auth,
            token,
            cookie,
            direct_download: direct,
            note,
        });
    }
    out
}

/// Catálogo completo: los repos por defecto + los del usuario (el usuario
/// puede sobrescribir uno por defecto usando su mismo `id`).
fn all_repos() -> Vec<RepoSpec> {
    let mut out = builtin_repos();
    for u in user_repos() {
        match out.iter_mut().find(|r| r.id == u.id) {
            Some(slot) => *slot = u,
            None => out.push(u),
        }
    }
    out
}

/// Busca una fuente por su identificador.
pub fn find_repo(repo_id: &str) -> Option<RepoSpec> {
    let id = repo_id.trim();
    if id.is_empty() {
        return None;
    }
    all_repos().into_iter().find(|r| r.id == id)
}

/// Fuentes que se usan por defecto en `plan()` / `watch_once()`: todas menos
/// las que no están configuradas (Play, genérico sin URL, Obtainium sin host).
fn default_sources() -> Vec<String> {
    all_repos()
        .into_iter()
        .filter(|r| r.configured && !r.index_urls.is_empty())
        .map(|r| r.id)
        .collect()
}

// ── Utilidades de lectura tolerante de JSON ─────────────────────────────────

/// Primer campo de texto no vacío entre `keys`.
fn first_str(v: &Value, keys: &[&str]) -> Option<String> {
    if !v.is_object() {
        return None;
    }
    for k in keys {
        if let Some(s) = v.get(*k).and_then(|x| x.as_str()) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// Primer campo numérico (entero) entre `keys`; acepta número o texto.
fn first_i64(v: &Value, keys: &[&str]) -> Option<i64> {
    if !v.is_object() {
        return None;
    }
    for k in keys {
        match v.get(*k) {
            Some(Value::Number(n)) => {
                if let Some(i) = n.as_i64() {
                    return Some(i);
                }
                if let Some(f) = n.as_f64() {
                    return Some(f as i64);
                }
            }
            Some(Value::String(s)) => {
                if let Ok(i) = s.trim().parse::<i64>() {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

// ── Normalización de índices ────────────────────────────────────────────────

/// Convierte epoch en milisegundos a fecha legible UTC.
fn epoch_ms_to_date(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let secs = ms.rem_euclid(86_400_000) / 1000;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Epoch (segundos) → ISO-8601 `YYYY-MM-DDTHH:MM:SSZ`.
fn epoch_to_iso(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Días desde 1970-01-01 → fecha civil (algoritmo de Howard Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Base URL a partir de la URL del índice (`…/repo/index-v1.json` → `…/repo/`).
fn base_url_from_index(index_url: &str) -> String {
    match index_url.rfind('/') {
        Some(i) => index_url[..=i].to_string(),
        None => String::new(),
    }
}

/// Hash válido (64 hexadecimales) o `None`.
///
/// Importante: algunos repos publishes SHA-1 en el campo `hash`; aceptar un
/// hash que no es SHA-256 haría que la verificación fallara SIEMPRE y el
/// daemon borraría APK legítimos.  Mejor no verificar que verificar mal.
fn valid_sha256(raw: Option<String>) -> Option<String> {
    let h = raw?.trim().to_lowercase();
    if h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(h)
    } else {
        None
    }
}

/// Normaliza UNA entrada cruda de cualquier repositorio.
///
/// Devuelve `None` si no se puede saber el paquete (sin `package` no hay
/// nada que cruzar).  El resto de campos ausentes se rellenan con valores
/// neutros: nunca se falla por un campo de menos.
fn normalize_entry(raw: &Value, base_url: &str) -> Option<Value> {
    if !raw.is_object() {
        return None;
    }
    let package = first_str(raw, &["packageName", "package", "package_id", "packageId", "appId"])?;
    let version_name =
        first_str(raw, &["versionName", "version_name", "version", "versionLabel"]).unwrap_or_default();
    let version_code = first_i64(raw, &["versionCode", "version_code", "versioncode"]).unwrap_or(-1);
    let apk_name = first_str(raw, &["apkName", "apk_name", "fileName", "filename"]);
    let explicit_url = first_str(raw, &["url", "apk_url", "apkUrl", "downloadUrl", "download_url"]);
    let sha256 = valid_sha256(first_str(raw, &["hash", "sha256", "sha_256", "sha256sum", "checksum"]));
    let size = first_i64(raw, &["size", "sizeBytes", "fileSize", "file_size"]).unwrap_or(0);
    let added_raw = first_str(raw, &["addedDate", "added_date", "added", "addedTime", "published"]);
    let added_epoch = first_i64(raw, &["addedDate", "added_date", "added", "addedTime", "timestamp"]);

    let added_date = match added_epoch {
        Some(ms) if ms > 0 => epoch_ms_to_date(ms),
        _ => added_raw.unwrap_or_default(),
    };

    // La URL del APK: explícita si el repo la da; si no, se deduce del
    // nombre de fichero y de la base del índice (así funciona F-Droid).
    let apk_url = match explicit_url {
        Some(u) => u,
        None => match apk_name {
            Some(n) if !base_url.is_empty() => format!("{}{}", base_url, n),
            _ => String::new(),
        },
    };

    Some(json!({
        "package": package,
        "version_name": version_name,
        "version_code": version_code,
        "apk_url": apk_url,
        "sha256": sha256.unwrap_or_default(),
        "size": size,
        "added_date": added_date,
    }))
}

/// Extrae la lista de entradas crudas de un índice F-Droid (`index-v1.json`).
///
/// Tolera las tres formas que se ven en la calle: `{"packages":[{…}]}`,
/// `{"packages":[[{…}]]}` (formato v2) y un array pelado.
fn fdroid_entries(body: &Value) -> Vec<Value> {
    let list = body
        .get("packages")
        .and_then(|p| p.as_array())
        .cloned()
        .or_else(|| body.as_array().cloned())
        .unwrap_or_default();
    let mut out = Vec::new();
    for item in list {
        match item {
            Value::Array(inner) => out.extend(inner.iter().cloned()),
            other => out.push(other),
        }
    }
    out
}

/// Extrae la lista de entradas crudas de un índice Obtainium / genérico.
fn generic_entries(body: &Value) -> Vec<Value> {
    if let Some(arr) = body.as_array() {
        return arr.clone();
    }
    for key in ["packages", "apps", "releases"] {
        if let Some(arr) = body.get(key).and_then(|p| p.as_array()) {
            let mut out = Vec::new();
            for item in arr {
                match item {
                    Value::Array(inner) => out.extend(inner.iter().cloned()),
                    other => out.push(other.clone()),
                }
            }
            return out;
        }
    }
    Vec::new()
}

/// Normaliza un índice completo al esquema interno.
fn normalize_index(spec: &RepoSpec, body: &Value, source_url: &str) -> Vec<Value> {
    let base = base_url_from_index(source_url);
    let raw = match spec.kind.as_str() {
        "fdroid" => fdroid_entries(body),
        _ => generic_entries(body),
    };
    let mut out = Vec::with_capacity(raw.len());
    for e in raw {
        if let Some(n) = normalize_entry(&e, &base) {
            out.push(n);
        }
    }
    out
}

// ── Caché en disco ──────────────────────────────────────────────────────────

/// Directorio de caché: `<state_dir>/repos`.
fn repos_dir() -> PathBuf {
    PathBuf::from(crate::cockpit_server::state_dir()).join("repos")
}

/// Ruta del fichero de caché de un repo.
fn cache_path(repo_id: &str) -> PathBuf {
    let safe: String = repo_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    repos_dir().join(format!("{}.json", safe))
}

/// Ruta del último plan calculado.
fn last_plan_path() -> PathBuf {
    repos_dir().join("last_plan.json")
}

/// Ahora mismo, en segundos epoch.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Escribe un fichero de forma atómica (escribe a `.tmp` y renombra).
fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("no se pudo crear {}: {}", parent.display(), e))?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text).map_err(|e| format!("no se pudo escribir {}: {}", tmp.display(), e))?;
    fs::rename(&tmp, path).map_err(|e| format!("no se pudo renombrar {}: {}", path.display(), e))
}

/// Guarda el índice normalizado de un repo en la caché.
fn write_cache(repo_id: &str, payload: &Value) -> Result<(), String> {
    let text = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    write_atomic(&cache_path(repo_id), &text)
}

/// Lee la caché de un repo (si existe).
fn read_cache(repo_id: &str) -> Option<Value> {
    let text = fs::read_to_string(cache_path(repo_id)).ok()?;
    serde_json::from_str::<Value>(&text).ok()
}

/// Antigüedad de la caché en segundos (`None` si no hay caché).
fn cache_age_secs(repo_id: &str) -> Option<u64> {
    let cached = read_cache(repo_id)?;
    let ts = cached.get("fetched_at").and_then(|v| v.as_u64())?;
    Some(now_secs().saturating_sub(ts))
}

/// Guarda el último plan para que `status()` pueda mostrarlo.
fn save_last_plan(plan: &Value) {
    if let Ok(text) = serde_json::to_string(plan) {
        let _ = write_atomic(&last_plan_path(), &text);
    }
}

/// Lee el último plan guardado.
fn read_last_plan() -> Option<Value> {
    let text = fs::read_to_string(last_plan_path()).ok()?;
    serde_json::from_str::<Value>(&text).ok()
}

// ── Capa HTTP ───────────────────────────────────────────────────────────────

/// Agente HTTP con timeouts cortos (nunca se queda colgado).
fn http_agent(timeout: Duration, connect: Duration) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .timeout_connect(Some(connect))
        .user_agent(USER_AGENT)
        .build();
    ureq::Agent::new_with_config(config)
}

/// Aplica la autenticación de la fuente (token o cookie) a la petición.
fn apply_auth<B>(req: ureq::RequestBuilder<B>, spec: &RepoSpec) -> ureq::RequestBuilder<B> {
    match spec.auth.as_str() {
        "token" if !spec.token.trim().is_empty() => {
            req.header("Authorization", &format!("Bearer {}", spec.token.trim()))
        }
        "cookie" if !spec.cookie.trim().is_empty() => req.header("Cookie", spec.cookie.trim()),
        _ => req,
    }
}

/// Descarga un recurso a memoria, con tope de tamaño.
///
/// Devuelve `(status, bytes)`.  Un 4xx/5xx NO es un error de ureq (viene
/// como `Err(StatusCode)`), se traduce a `Ok((status, vacío))` para poder
///Diagnostics: "HTTP 403" en vez de un error genérico.
fn http_get_bytes(url: &str, spec: &RepoSpec, max_bytes: u64, timeout: Duration) -> Result<(u16, Vec<u8>), String> {
    let agent = http_agent(timeout, Duration::from_secs(CONNECT_TIMEOUT_SECS));
    let req = apply_auth(agent.get(url), spec);
    match req.call() {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let bytes = resp
                .into_body()
                .into_with_config()
                .limit(max_bytes)
                .read_to_vec()
                .map_err(|e| format!("cuerpo ilegible: {}", e))?;
            Ok((status, bytes))
        }
        Err(ureq::Error::StatusCode(code)) => Ok((code, Vec::new())),
        Err(e) => Err(e.to_string()),
    }
}

/// Descarga un APK a disco (streaming, con tope de tamaño).
///
/// Devuelve el número de bytes escritos.  Si algo falla, el fichero parcial
/// se borra: nunca dejamos un `.apk` a medias.
fn http_download_to_file(url: &str, spec: &RepoSpec, dest: &Path, max_bytes: u64) -> Result<u64, String> {
    let agent = http_agent(
        Duration::from_secs(APK_TIMEOUT_SECS),
        Duration::from_secs(CONNECT_TIMEOUT_SECS),
    );
    let req = apply_auth(agent.get(url), spec);
    let resp = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(code)) => return Err(format!("HTTP {}", code)),
        Err(e) => return Err(e.to_string()),
    };
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {}", status));
    }
    let mut reader = resp.into_body().into_with_config().limit(max_bytes).reader();
    let file = fs::File::create(dest).map_err(|e| format!("no se pudo crear {}: {}", dest.display(), e))?;
    let mut writer = std::io::BufWriter::new(file);
    let written = match std::io::copy(&mut reader, &mut writer) {
        Ok(n) => n,
        Err(e) => {
            let _ = fs::remove_file(dest);
            return Err(format!("descarga interrumpida: {}", e));
        }
    };
    if let Err(e) = writer.flush() {
        let _ = fs::remove_file(dest);
        return Err(format!("no se pudo volcar el APK: {}", e));
    }
    Ok(written)
}

// ── Inventario local de apps ───────────────────────────────────────────────

/// Inventario de apps instaladas (delegado en `apk_inventory`).
///
/// Se encapsula aquí para tener UN solo punto de contacto con el módulo de
/// inventario: si su API cambia, sólo hay que tocar esta función.
fn inventory_json(limit: usize) -> Value {
    crate::apk_inventory::inventory(limit)
}

/// Apps instaladas en el formato interno del plan.
///
/// Preferencia: el inventario de `apk_inventory`.  Si ese módulo todavía no
/// está implementado (o no devuelve apps), se cae a `dumpsys package
/// packages`, que siempre está en un Android con root.  Si tampoco funciona
/// (PC Linux sin Android), se devuelve una lista vacía y un aviso: el daemon
/// degrada, no rompe.
fn installed_apps() -> Vec<Value> {
    let raw = inventory_json(DEFAULT_APP_LIMIT);
    let mut out: Vec<Value> = Vec::new();
    if let Some(list) = raw.get("packages").and_then(|p| p.as_array()) {
        for item in list {
            if let Some(e) = normalize_installed(item) {
                out.push(e);
            }
        }
    }
    if !out.is_empty() {
        return out;
    }
    // Fallback: `dumpsys package packages`.
    match crate::android::root_exec("dumpsys package packages", PM_TIMEOUT_MS, PM_MAX_CHARS) {
        r if r.code == Some(0) => parse_dumpsys_packages(&r.stdout),
        r => {
            let _ = r;
            Vec::new()
        }
    }
}

/// Normaliza una entrada del inventario local.
fn normalize_installed(item: &Value) -> Option<Value> {
    if !item.is_object() {
        return None;
    }
    let package = first_str(item, &["package", "packageName", "package_name", "id"])?;
    let version_name = first_str(item, &["version_name", "versionName", "version"]).unwrap_or_default();
    let version_code = first_i64(item, &["version_code", "versionCode"]).unwrap_or(-1);
    let apk_path = first_str(item, &["apk_path", "apkPath", "path"]).unwrap_or_default();
    let sha256 = first_str(item, &["sha256", "hash"]).unwrap_or_default();
    Some(json!({
        "package": package,
        "version_name": version_name,
        "version_code": version_code,
        "apk_path": apk_path,
        "sha256": sha256,
    }))
}

/// Parsea la salida de `dumpsys package packages`.
///
/// Formato por bloque:
/// ```text
///   Package [org.fdroid.fdroid] (1a2b3c):
///     codePath=/data/app/~~xyz/org.fdroid.fdroid-hash
///     versionCode=10800 minSdk=23 targetSdk=34
///     versionName=0.108.0
/// ```
fn parse_dumpsys_packages(text: &str) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut pkg = String::new();
    let mut code: i64 = -1;
    let mut name = String::new();
    let mut path = String::new();

    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("Package [") {
            // Flush del bloque anterior.
            if !pkg.is_empty() {
                out.push(json!({
                    "package": pkg,
                    "version_name": name,
                    "version_code": code,
                    "apk_path": path,
                    "sha256": "",
                }));
            }
            pkg = String::new();
            code = -1;
            name = String::new();
            path = String::new();
            if let Some(start) = t.find('[') {
                if let Some(end) = t[start + 1..].find(']') {
                    pkg = t[start + 1..start + 1 + end].to_string();
                }
            }
            continue;
        }
        if pkg.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("versionCode=") {
            let num: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
            code = num.parse::<i64>().unwrap_or(-1);
        } else if let Some(rest) = t.strip_prefix("versionName=") {
            name = rest.split_whitespace().next().unwrap_or("").to_string();
        } else if let Some(rest) = t.strip_prefix("codePath=") {
            path = rest.split_whitespace().next().unwrap_or("").to_string();
        }
    }
    if !pkg.is_empty() {
        out.push(json!({
            "package": pkg,
            "version_name": name,
            "version_code": code,
            "apk_path": path,
            "sha256": "",
        }));
    }
    out
}

// ── Lógica de cruce (función pura) ──────────────────────────────────────────

/// Cruza el inventario local con los índices de los repositorios y devuelve
/// el plan de actualización.
///
/// **Función pura**: no toca disco ni red, sólo recibe datos y devuelve el
/// plan.  Eso la hace trivialmente testeable.
///
/// `installed`: apps instaladas (`package`, `version_name`, `version_code`…).
/// `indexes`:   índices ya normalizados.  Cada elemento puede ser
///              `{ "source": "fdroid", "packages": [ … ] }` o directamente
///              una entrada normalizada (con `source` dentro).
pub fn build_plan(installed: &[Value], indexes: &[Value]) -> Value {
    // 1) Aplanar los índices: paquete → (entrada, fuente).
    //    Si dos repos tienen la misma app, gana el de mayor versionCode.
    let mut available: std::collections::HashMap<String, (Value, String)> = std::collections::HashMap::new();
    for idx in indexes {
        let (source, list) = flatten_index(idx);
        for e in list {
            let pkg = match e.get("package").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => continue,
            };
            let better = match available.get(&pkg) {
                None => true,
                Some((cur, _)) => version_code_of(&e) > version_code_of(cur),
            };
            if better {
                available.insert(pkg, (e.clone(), source.clone()));
            }
        }
    }

    // 2) Recorrer las apps instaladas.
    let mut updates: Vec<Value> = Vec::new();
    let mut blocked_items: Vec<Value> = Vec::new();
    let mut watched = 0usize;
    let mut evaluated = 0usize;

    for app in installed {
        let pkg = match app.get("package").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => p.to_string(),
            _ => continue,
        };
        watched += 1;
        let (entry, source) = match available.get(&pkg) {
            Some(x) => x.clone(),
            None => continue, // la app no está en ningún repo: no hay nada que hacer
        };
        evaluated += 1;

        let installed_code = version_code_of(app);
        let available_code = version_code_of(&entry);
        let installed_name = version_name_of(app);
        let available_name = version_name_of(&entry);
        let apk_url = entry
            .get("apk_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let sha256 = entry
            .get("sha256")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let size = entry.get("size").and_then(|v| v.as_i64()).unwrap_or(0);

        // Versión no comparable (falta versionCode): no se inventa nada.
        if installed_code < 0 || available_code < 0 {
            let reason = "no se puede comparar la versión (falta versionCode)".to_string();
            blocked_items.push(json!({
                "package": pkg,
                "reason": reason,
                "risk": "medium",
                "installed_version": installed_name,
                "available_version": available_name,
                "source": source,
            }));
            continue;
        }

        // Downgrade: el repo ofrece una versión MÁS VIEJA.  Riesgo alto y,
        // por defecto, bloqueado (instalar un downgrade rompe la app).
        if available_code < installed_code {
            let reason = format!(
                "downgrade: el repo ofrece versionCode {} y la instalada es {}",
                available_code, installed_code
            );
            let item = json!({
                "package": pkg,
                "installed_version": installed_name,
                "installed_version_code": installed_code,
                "available_version": available_name,
                "available_version_code": available_code,
                "apk_url": apk_url,
                "size": size,
                "sha256": sha256,
                "source": source,
                "action": "download",
                "risk": "high",
                "blocked": true,
                "reason": reason,
            });
            blocked_items.push(item.clone());
            updates.push(item);
            continue;
        }

        // Misma versión: la app NO aparece en el plan.
        if available_code == installed_code {
            continue;
        }

        // Hay versión nueva: se calcula el riesgo.
        let is_system = app
            .get("apk_path")
            .and_then(|v| v.as_str())
            .map(|p| p.starts_with("/system") || p.starts_with("/vendor") || p.starts_with("/product"))
            .unwrap_or(false);
        let risk = if sha256.is_empty() {
            // Sin hash no se puede verificar la integridad antes de instalar.
            "medium"
        } else if is_system {
            // Actualizar una app del sistema puede dejar el móvil inconsistente.
            "medium"
        } else {
            "low"
        };
        let blocked = apk_url.is_empty();
        let reason = if blocked {
            "la fuente no ofrece descarga directa: hay que instalarla a mano".to_string()
        } else {
            String::new()
        };
        let item = json!({
            "package": pkg,
            "installed_version": installed_name,
            "installed_version_code": installed_code,
            "available_version": available_name,
            "available_version_code": available_code,
            "apk_url": apk_url,
            "size": size,
            "sha256": sha256,
            "source": source,
            "action": if blocked { "install" } else { "download" },
            "risk": risk,
            "blocked": blocked,
            "reason": reason,
        });
        if blocked {
            blocked_items.push(item.clone());
        }
        updates.push(item);
    }

    let updatable = updates
        .iter()
        .filter(|u| !u.get("blocked").and_then(|b| b.as_bool()).unwrap_or(false))
        .count();
    let blocked = blocked_items.len();

    json!({
        "ok": true,
        "total": evaluated,
        "watched": watched,
        "updatable": updatable,
        "blocked": blocked,
        "updates": updates,
        "blocked_items": blocked_items,
    })
}

/// Aplana un índice de entrada para `build_plan`.
///
/// Acepta `{source, packages:[…]}` o una entrada suelta con `source` dentro.
fn flatten_index(idx: &Value) -> (String, Vec<Value>) {
    let source = idx
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if let Some(list) = idx.get("packages").and_then(|p| p.as_array()) {
        return (source, list.clone());
    }
    (source, vec![idx.clone()])
}

/// `version_code` de una entrada (o -1 si no se sabe).
fn version_code_of(v: &Value) -> i64 {
    v.get("version_code")
        .and_then(|x| x.as_i64())
        .or_else(|| first_i64(v, &["versionCode", "version_code"]))
        .unwrap_or(-1)
}

/// `version_name` de una entrada (o cadena vacía).
fn version_name_of(v: &Value) -> String {
    v.get("version_name")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .or_else(|| first_str(v, &["versionName", "version_name"]))
        .unwrap_or_default()
}

// ── API pública ─────────────────────────────────────────────────────────────

/// Catálogo de fuentes de apps.
///
/// Cada entrada lleva: id, nombre, tipo, URL del índice, autenticación y si
/// soporta descarga directa.  Play aparece, pero marcado como NO utilizable
/// (sin API pública) en lugar de inventarse un endpoint.
pub fn index() -> Value {
    repos()
}

/// Catálogo de repositorios soportados.
pub fn repos() -> Value {
    let all = all_repos();
    let mut list = Vec::new();
    for r in &all {
        let mut item = r.to_json();
        let age = cache_age_secs(&r.id);
        let cached = read_cache(&r.id);
        let count = cached
            .as_ref()
            .and_then(|c| c.get("count").and_then(|v| v.as_u64()))
            .unwrap_or(0);
        if let Some(obj) = item.as_object_mut() {
            obj.insert("cached".into(), json!(age.is_some()));
            obj.insert("cache_age_secs".into(), json!(age));
            obj.insert("cached_count".into(), json!(count));
            obj.insert("cache_path".into(), json!(cache_path(&r.id).to_string_lossy()));
            obj.insert("usable".into(), json!(r.configured && !r.index_urls.is_empty()));
        }
        list.push(item);
    }
    json!({
        "ok": true,
        "count": list.len(),
        "repos": list,
        "state_dir": crate::cockpit_server::state_dir(),
    })
}

/// Descarga (o sirve de caché) el índice de un repositorio.
///
/// Devuelve `{ ok, repo, count, packages:[…] }` con las entradas ya
/// normalizadas.  Si la red falla pero hay caché, responde `ok:true` con
/// `stale:true` y el motivo del fallo en `error`.
pub fn fetch_index(repo_id: &str) -> Value {
    let spec = match find_repo(repo_id) {
        Some(s) => s,
        None => {
            return json!({
                "ok": false,
                "repo": repo_id,
                "count": 0,
                "packages": [],
                "stale": false,
                "error": format!("fuente desconocida: {}", repo_id),
            })
        }
    };

    if spec.index_urls.is_empty() {
        return json!({
            "ok": false,
            "repo": spec.id,
            "count": 0,
            "packages": [],
            "stale": false,
            "error": spec.note,
            "requires_account": spec.kind == "play",
            "requires_external_installer": spec.kind == "play",
        });
    }

    let timeout = Duration::from_secs(INDEX_TIMEOUT_SECS);
    let mut last_err = String::from("no se pudo contactar con ninguna URL");
    for url in &spec.index_urls {
        match http_get_bytes(url, &spec, MAX_INDEX_BYTES, timeout) {
            Ok((status, bytes)) if (200..300).contains(&status) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(body) => {
                    let packages = normalize_index(&spec, &body, url);
                    let now = now_secs();
                    let payload = json!({
                        "repo": spec.id,
                        "source_url": url,
                        "fetched_at": now,
                        "timestamp": epoch_to_iso(now),
                        "count": packages.len(),
                        "packages": packages,
                    });
                    let _ = write_cache(&spec.id, &payload);
                    return json!({
                        "ok": true,
                        "repo": spec.id,
                        "count": packages.len(),
                        "packages": packages,
                        "stale": false,
                        "fetched_at": now,
                        "timestamp": epoch_to_iso(now),
                        "source_url": url,
                        "cache_path": cache_path(&spec.id).to_string_lossy(),
                        "error": Value::Null,
                    });
                }
                Err(e) => {
                    last_err = format!("JSON inválido en {}: {}", url, e);
                }
            },
            Ok((status, _)) => {
                last_err = format!("HTTP {} en {}", status, url);
            }
            Err(e) => {
                last_err = format!("{}: {}", url, e);
            }
        }
    }

    // Sin red: caché (si la hay).
    if let Some(cached) = read_cache(&spec.id) {
        let packages = cached.get("packages").and_then(|p| p.as_array()).cloned().unwrap_or_default();
        let count = packages.len();
        return json!({
            "ok": true,
            "repo": spec.id,
            "count": count,
            "packages": packages,
            "stale": true,
            "fetched_at": cached.get("fetched_at").cloned().unwrap_or(Value::Null),
            "source_url": cached.get("source_url").cloned().unwrap_or(Value::Null),
            "error": format!("red no disponible ({}); se sirve la caché", last_err),
        });
    }

    json!({
        "ok": false,
        "repo": spec.id,
        "count": 0,
        "packages": [],
        "stale": false,
        "error": last_err,
    })
}

/// Índice de un repo, usando la caché si es fresca y bajándolo si hace falta.
///
/// Devuelve `(entradas, stale)`.
fn load_index(repo_id: &str) -> Result<(Vec<Value>, bool), String> {
    // 1) Caché fresca: no se toca la red.
    if let Some(cached) = read_cache(repo_id) {
        if let Some(ts) = cached.get("fetched_at").and_then(|v| v.as_u64()) {
            if now_secs().saturating_sub(ts) < CACHE_TTL_SECS {
                let list = cached
                    .get("packages")
                    .and_then(|p| p.as_array())
                    .cloned()
                    .unwrap_or_default();
                return Ok((list, false));
            }
        }
    }
    // 2) Red (con caída a caché si falla).
    let res = fetch_index(repo_id);
    if res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        let list = res
            .get("packages")
            .and_then(|p| p.as_array())
            .cloned()
            .unwrap_or_default();
        let stale = res.get("stale").and_then(|v| v.as_bool()).unwrap_or(false);
        return Ok((list, stale));
    }
    Err(res
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("índice no disponible")
        .to_string())
}

/// Plan de actualización: cruza los índices con las apps instaladas.
///
/// `sources`: `None` = todas las fuentes configuradas; `Some(vec)` = sólo
/// esas (las desconocidas se reportan en `errors`).
///
/// Salida:
///   * `total`     — apps evaluadas (instaladas y presentes en algún repo)
///   * `watched`   — apps instaladas consideradas
///   * `updatable` — actualizaciones accionables (riesgo medium/low, sin bloqueo)
///   * `blocked`   — actualizaciones bloqueadas (con `reason`)
pub fn plan(sources: Option<Vec<String>>) -> Value {
    let wanted: Vec<String> = match sources {
        Some(list) => list,
        None => default_sources(),
    };

    let mut indexes: Vec<Value> = Vec::new();
    let mut used: Vec<String> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();

    for id in &wanted {
        match load_index(id) {
            Ok((list, stale)) => {
                indexes.push(json!({
                    "source": id,
                    "stale": stale,
                    "packages": list,
                }));
                used.push(id.clone());
            }
            Err(e) => errors.push(json!({ "repo": id, "error": e })),
        }
    }

    let installed = installed_apps();
    let mut plan = build_plan(&installed, &indexes);
    if let Some(obj) = plan.as_object_mut() {
        obj.insert("sources".into(), json!(used));
        obj.insert("errors".into(), json!(errors));
        obj.insert("generated_at".into(), json!(now_secs()));
        obj.insert("timestamp".into(), json!(epoch_to_iso(now_secs())));
    }
    save_last_plan(&plan);
    plan
}

/// Descarga el APK de un paquete desde un repositorio.
///
/// Verifica el SHA-256 contra el hash del índice.  **Si no cuadra, borra el
/// fichero y devuelve error**: un APK manipulado no llega nunca a `pm`.
pub fn download(package: &str, repo_id: &str, dest_dir: &str) -> Value {
    let pkg = package.trim();
    if pkg.is_empty() {
        return json!({"ok": false, "package": package, "error": "falta el nombre del paquete"});
    }
    let spec = match find_repo(repo_id) {
        Some(s) => s,
        None => {
            return json!({
                "ok": false,
                "package": pkg,
                "repo": repo_id,
                "error": format!("fuente desconocida: {}", repo_id),
            })
        }
    };

    // Entrada del paquete en el índice (caché primero, red si hace falta).
    let (entries, stale) = match load_index(&spec.id) {
        Ok(v) => v,
        Err(e) => {
            return json!({
                "ok": false,
                "package": pkg,
                "repo": spec.id,
                "error": e,
            })
        }
    };
    let entry = entries
        .iter()
        .find(|e| e.get("package").and_then(|v| v.as_str()) == Some(pkg))
        .cloned();
    let entry = match entry {
        Some(e) => e,
        None => {
            return json!({
                "ok": false,
                "package": pkg,
                "repo": spec.id,
                "error": format!("el paquete {} no está en el índice de {}", pkg, spec.id),
                "stale_index": stale,
            })
        }
    };

    let apk_url = entry
        .get("apk_url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if apk_url.is_empty() {
        return json!({
            "ok": false,
            "package": pkg,
            "repo": spec.id,
            "error": "la fuente no ofrece URL de descarga directa para este paquete",
        });
    }

    let dest = if dest_dir.trim().is_empty() {
        PathBuf::from(crate::cockpit_server::state_dir()).join("downloads")
    } else {
        PathBuf::from(dest_dir.trim())
    };
    if let Err(e) = fs::create_dir_all(&dest) {
        return json!({
            "ok": false,
            "package": pkg,
            "repo": spec.id,
            "error": format!("no se pudo crear {}: {}", dest.display(), e),
        });
    }
    let final_path = dest.join(format!("{}.apk", safe_file_name(pkg)));
    let tmp_path = dest.join(format!("{}.apk.part", safe_file_name(pkg)));

    // 1) Descargar a un temporal.
    let downloaded = match http_download_to_file(&apk_url, &spec, &tmp_path, MAX_APK_BYTES) {
        Ok(n) => n,
        Err(e) => {
            let _ = fs::remove_file(&tmp_path);
            return json!({
                "ok": false,
                "package": pkg,
                "repo": spec.id,
                "error": format!("fallo la descarga: {}", e),
                "apk_url": apk_url,
            });
        }
    };

    // 2) Verificar el hash y renombrar (o borrar).
    let expected = entry
        .get("sha256")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    finalize_download(&tmp_path, &final_path, &expected, downloaded)
}

/// Verifica el SHA-256 de un APK descargado y lo deja en su sitio final.
///
/// Función aparte (y sin red) para poder testearla: si el hash NO cuadra, el
/// fichero se BORRA y se devuelve el error.
fn finalize_download(tmp_path: &Path, final_path: &Path, expected_sha256: &str, downloaded: u64) -> Value {
    let (digest, size) = match sha256_file(tmp_path) {
        Ok(v) => v,
        Err(e) => {
            let _ = fs::remove_file(tmp_path);
            return json!({
                "ok": false,
                "path": Value::Null,
                "size": downloaded,
                "sha256": Value::Null,
                "verified": false,
                "error": format!("no se pudo calcular el SHA-256: {}", e),
            });
        }
    };

    let expected = expected_sha256.trim().to_lowercase();
    if !expected.is_empty() && expected != digest {
        // Hash incorrecto: el fichero se borra.  No se instala nada dudoso.
        let _ = fs::remove_file(tmp_path);
        return json!({
            "ok": false,
            "path": Value::Null,
            "size": size,
            "sha256": digest,
            "expected_sha256": expected,
            "verified": false,
            "error": format!("SHA-256 no coincide: esperado {}, obtenido {}", expected, digest),
        });
    }

    if let Err(e) = fs::rename(tmp_path, final_path) {
        let _ = fs::remove_file(tmp_path);
        return json!({
            "ok": false,
            "path": Value::Null,
            "size": size,
            "sha256": digest,
            "verified": false,
            "error": format!("no se pudo renombrar el APK: {}", e),
        });
    }

    json!({
        "ok": true,
        "path": final_path.to_string_lossy(),
        "size": size,
        "sha256": digest,
        "verified": !expected.is_empty(),
        "error": Value::Null,
        "note": if expected.is_empty() {
            "la fuente no publica hash: la integridad no se ha podido verificar"
        } else {
            "SHA-256 verificado contra el índice"
        },
    })
}

/// SHA-256 de un fichero (en streaming) + tamaño en bytes.
fn sha256_file(path: &Path) -> Result<(String, u64), String> {
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    let out = hasher.finalize();
    Ok((format!("{:x}", out), total))
}

/// Nombre de fichero seguro a partir de un paquete.
fn safe_file_name(pkg: &str) -> String {
    pkg.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Instala un APK con `pm install -r -g` (como root).
///
/// **Exige `confirm == true`**: sin confirmación no se ejecuta NADA (ni
/// siquiera se comprueba el fichero), que es lo que evita que un agente se
/// instale solo una app.
pub fn apply(package: &str, apk_path: &str, confirm: bool) -> Value {
    if !confirm {
        return json!({
            "ok": false,
            "package": package,
            "apk_path": apk_path,
            "installed": false,
            "error": "confirm debe ser true: no se instala nada sin confirmación explícita",
            "hint": "repite la llamada con confirm=true",
        });
    }

    let path = Path::new(apk_path.trim());
    if !path.exists() {
        return json!({
            "ok": false,
            "package": package,
            "apk_path": apk_path,
            "installed": false,
            "error": format!("no existe el APK: {}", apk_path),
        });
    }

    // `-r` reemplaza la versión instalada, `-g` concede todos los permisos en
    // tiempo de instalación.  La ruta va entrecomillada (sh_quote) para que
    // ningún nombre de fichero pueda inyectar comandos.
    let cmd = format!("pm install -r -g {}", crate::android::sh_quote(&path.to_string_lossy()));
    let res = crate::android::root_exec(&cmd, 300_000, 64 * 1024);
    let ok = res.code == Some(0) && !res.timed_out;

    json!({
        "ok": ok,
        "package": package,
        "apk_path": apk_path,
        "installed": ok,
        "exit_code": res.code,
        "timed_out": res.timed_out,
        "duration_ms": res.duration_ms,
        "output": res.combined(),
        "error": if ok {
            Value::Null
        } else if res.timed_out {
            json!("timeout instalando el APK")
        } else {
            json!(format!("pm install falló (código {:?})", res.code))
        },
    })
}

/// Estado global: fuentes, edad de los índices, caché, apps vigiladas,
/// actualizaciones pendientes y último plan.
pub fn status() -> Value {
    let catalog = repos();
    let repos_list = catalog.get("repos").and_then(|r| r.as_array()).cloned().unwrap_or_default();

    // Edad del último índice (el más reciente entre todas las fuentes).
    let mut ages: Vec<u64> = repos_list
        .iter()
        .filter_map(|r| r.get("cache_age_secs").and_then(|v| v.as_u64()))
        .collect();
    ages.sort_unstable();
    let last_index_age = ages.first().copied();

    // apps vigiladas
    let installed = installed_apps();
    let watched = installed.len();

    // último plan
    let last_plan = read_last_plan();
    let pending = last_plan
        .as_ref()
        .and_then(|p| p.get("updatable").and_then(|v| v.as_u64()))
        .unwrap_or(0);
    let blocked = last_plan
        .as_ref()
        .and_then(|p| p.get("blocked").and_then(|v| v.as_u64()))
        .unwrap_or(0);

    // caché en disco
    let mut cache_entries = Vec::new();
    let mut cache_bytes: u64 = 0;
    if let Ok(rd) = fs::read_dir(repos_dir()) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let size = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            cache_bytes += size;
            cache_entries.push(json!({
                "file": p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
                "size": size,
            }));
        }
    }

    json!({
        "ok": true,
        "repos": repos_list,
        "repos_count": repos_list.len(),
        "last_index_age_secs": last_index_age,
        "cache": {
            "dir": repos_dir().to_string_lossy(),
            "entries": cache_entries,
            "total_bytes": cache_bytes,
        },
        "watched": watched,
        "pending_updates": pending,
        "blocked_updates": blocked,
        "last_plan": last_plan,
        "generated_at": now_secs(),
        "timestamp": epoch_to_iso(now_secs()),
    })
}

/// Ciclo NO destructivo: actualiza los índices y recalcula el plan.
///
/// No descarga ni instala NADA: sólo lee.  Es el ciclo que el daemon puede
/// repetir cada hora sin riesgo.
pub fn watch_once() -> Value {
    let mut fetched: Vec<Value> = Vec::new();
    let mut indexes: Vec<Value> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();

    for id in default_sources() {
        let res = fetch_index(&id);
        let ok = res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        if ok {
            let packages = res.get("packages").and_then(|p| p.as_array()).cloned().unwrap_or_default();
            fetched.push(json!({
                "repo": id,
                "ok": true,
                "count": packages.len(),
                "stale": res.get("stale").cloned().unwrap_or(json!(false)),
            }));
            indexes.push(json!({
                "source": id,
                "packages": packages,
            }));
        } else {
            let err = res.get("error").cloned().unwrap_or(json!("error desconocido"));
            errors.push(json!({ "repo": id, "error": err }));
            fetched.push(json!({ "repo": id, "ok": false, "error": err }));
        }
    }

    let installed = installed_apps();
    let mut plan = build_plan(&installed, &indexes);
    if let Some(obj) = plan.as_object_mut() {
        obj.insert("sources".into(), json!(fetched));
        obj.insert("errors".into(), json!(errors));
        obj.insert("generated_at".into(), json!(now_secs()));
        obj.insert("timestamp".into(), json!(epoch_to_iso(now_secs())));
    }
    save_last_plan(&plan);

    json!({
        "ok": true,
        "destructive": false,
        "fetched": fetched,
        "errors": errors,
        "watched": installed.len(),
        "updatable": plan.get("updatable").cloned().unwrap_or(json!(0)),
        "blocked": plan.get("blocked").cloned().unwrap_or(json!(0)),
        "plan": plan,
    })
}

// ── Despliegue del daemon en el dispositivo (sin adb) ──────────────────────

/// Ruta habitual del binario del daemon en Android.
pub const DEFAULT_BIN_PATH: &str = "/data/local/tmp/megadaemon/megadaemon-core";
/// Ruta habitual del supervisor Magisk.
pub const DEFAULT_SERVICE_PATH: &str = "/data/adb/service.d/99-megadaemon.sh";
/// Puerto del cockpit en el móvil.
pub const COCKPIT_PORT: u16 = 9090;
/// Puerto de ADB sobre TCP.
pub const ADB_TCP_PORT: u16 = 5555;

/// Comprobación de salud del cockpit: devuelve si responde y el código HTTP.
///
/// El sondeo se hace DENTRO del dispositivo (una sola llamada a root) para
/// no gastar 45 viajes de red: espera hasta 90 s a que el daemon levante.
fn health_probe_cmd(port: u16) -> String {
    format!(
        "i=0; code=000; while [ $i -lt 45 ]; do \
           if command -v curl >/dev/null 2>&1; then \
             code=$(curl -s -m 2 -o /dev/null -w '%{{http_code}}' http://127.0.0.1:{port}/health 2>/dev/null); \
           else \
             code=$(toybox wget -q -O /dev/null http://127.0.0.1:{port}/health 2>/dev/null && echo 200 || echo 000); \
           fi; \
           [ \"$code\" = 200 ] && break; i=$((i+1)); sleep 2; done; echo HEALTH:$code",
        port = port
    )
}

/// Interpreta la salida del sondeo de salud.
///
/// Función pura (para testearla sin dispositivo): "HEALTH:200" → sana.
fn parse_health_output(out: &str) -> (bool, String) {
    for line in out.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("HEALTH:") {
            let code = rest.trim().to_string();
            let ok = code == "200";
            return (ok, code);
        }
    }
    (false, "000".to_string())
}

/// Estado de salud del cockpit en el dispositivo.
pub fn health(port: u16) -> Value {
    let res = crate::android::root_exec(&health_probe_cmd(port), 120_000, 4096);
    let (ok, code) = parse_health_output(&res.combined());
    let alive = process_alive("megadaemon-core");
    json!({
        "ok": ok,
        "port": port,
        "http_code": code,
        "process_alive": alive,
        "error": if ok { Value::Null } else { json!("el cockpit no responde en el puerto indicado") },
    })
}

/// ¿Vive el proceso? (por nombre, sin depender de `pidof`).
fn process_alive(name: &str) -> bool {
    let cmd = format!("pgrep -f {} >/dev/null 2>&1 && echo YES || echo NO", name);
    let res = crate::android::root_exec(&cmd, 15_000, 1024);
    res.stdout.contains("YES")
}

/// Reinicia el daemon: mata el proceso y se asegura de que el supervisor
/// Magisk está vivo (si ya lo está, él solo lo relanza).
fn restart_daemon() -> String {
    let cmd = format!(
        "killall megadaemon-core 2>/dev/null; sleep 1; \
         pgrep -f 99-megadaemon >/dev/null 2>&1 || \
         setsid nohup sh {} >/dev/null 2>&1 & sleep 1; echo RESTARTED",
        crate::android::sh_quote(DEFAULT_SERVICE_PATH)
    );
    let res = crate::android::root_exec(&cmd, 30_000, 4096);
    res.combined()
}

/// Despliega un binario nuevo del daemon en el dispositivo.
///
/// Camino completo, **sin adb**: el fichero ya descargado y verificado
/// (`download()`) se copia sobre el binario en uso (guardando el anterior),
/// se reinicia el daemon y se comprueba `/health`.  Si el daemon no levanta,
/// se **revierte** al binario anterior y se informa del rollback.
///
/// `confirm == true` es obligatorio: esto sustituye el daemon en marcha.
pub fn deploy_binary(package: &str, src_path: &str, target_path: &str, confirm: bool) -> Value {
    if !confirm {
        return json!({
            "ok": false,
            "package": package,
            "installed": false,
            "error": "confirm debe ser true: no se sustituye el daemon sin confirmación explícita",
            "hint": "repite la llamada con confirm=true",
        });
    }
    let src = Path::new(src_path.trim());
    if !src.exists() {
        return json!({
            "ok": false,
            "package": package,
            "installed": false,
            "error": format!("no existe el fichero a desplegar: {}", src_path),
        });
    }
    let target = if target_path.trim().is_empty() {
        DEFAULT_BIN_PATH.to_string()
    } else {
        target_path.trim().to_string()
    };

    let (digest, size) = match sha256_file(src) {
        Ok(v) => v,
        Err(e) => {
            return json!({
                "ok": false,
                "package": package,
                "installed": false,
                "error": format!("no se pudo calcular el SHA-256: {}", e),
            })
        }
    };

    // 1) Copia del binario actual (para poder revertir).
    let backup = format!("{}.bak", target);
    let _ = crate::android::root_exec(
        &format!(
            "cp -f {} {} 2>/dev/null",
            crate::android::sh_quote(&target),
            crate::android::sh_quote(&backup)
        ),
        30_000,
        4096,
    );

    // 2) Instalación del binario nuevo.
    let install = crate::android::root_exec(
        &format!(
            "cp -f {} {} && chmod 755 {}",
            crate::android::sh_quote(&src.to_string_lossy()),
            crate::android::sh_quote(&target),
            crate::android::sh_quote(&target)
        ),
        60_000,
        4096,
    );
    if install.code != Some(0) {
        return json!({
            "ok": false,
            "package": package,
            "installed": false,
            "error": "no se pudo copiar el binario al dispositivo",
            "output": install.combined(),
        });
    }

    // 3) Reinicio + salud.
    let restart = restart_daemon();
    let probe = health(COCKPIT_PORT);

    if probe.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return json!({
            "ok": true,
            "package": package,
            "installed": true,
            "path": target,
            "size": size,
            "sha256": digest,
            "health": probe,
            "restart": restart,
            "error": Value::Null,
        });
    }

    // 4) Rollback: el binario nuevo no levantaba el daemon.
    let _ = crate::android::root_exec(
        &format!(
            "cp -f {} {} && chmod 755 {}",
            crate::android::sh_quote(&backup),
            crate::android::sh_quote(&target),
            crate::android::sh_quote(&target)
        ),
        60_000,
        4096,
    );
    let _ = restart_daemon();
    json!({
        "ok": false,
        "package": package,
        "installed": false,
        "rolled_back": true,
        "path": target,
        "size": size,
        "sha256": digest,
        "health": probe,
        "error": "el binario nuevo no arrancó el cockpit: se ha revertido al anterior",
    })
}

// ── ADB remoto (TCP) para todos los conectados ──────────────────────────────

/// Estado de ADB sobre TCP en el dispositivo.
pub fn adb_status() -> Value {
    let cmd = "port=$(getprop service.adb.tcp.port 2>/dev/null); \
               adbd=$(pgrep -f adbd >/dev/null 2>&1 && echo yes || echo no); \
               ips=$(ip -4 -o addr show 2>/dev/null | awk '{print $2}' | cut -d/ -f1 | tr '\\n' ',' ); \
               echo PORT:$port; echo ADBD:$adbd; echo IPS:$ips";
    let res = crate::android::root_exec(cmd, 20_000, 4096);
    let text = res.combined();
    let port = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("PORT:").map(|v| v.trim().to_string()))
        .unwrap_or_default();
    let adbd = text.contains("ADBD:yes");
    let ips: Vec<String> = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("IPS:").map(|v| v.to_string()))
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let enabled = port.parse::<i64>().map(|p| p > 0).unwrap_or(false);
    json!({
        "ok": true,
        "adb_tcp_port": port,
        "enabled": enabled,
        "adbd_running": adbd,
        "ips": ips,
        "hint": if enabled {
            "desde un PC: adb connect <ip>:<puerto> (varios equipos a la vez)"
        } else {
            "ADB por TCP desactivado: actívalo con remote_adb(true, confirm=true)"
        },
    })
}

/// Activa o desactiva **ADB sobre TCP** para que varios equipos a la vez
/// (PC, otro móvil, la red de casa) puedan conectar con `adb connect`.
///
/// AVISO DE SEGURIDAD: `adbd` por TCP da control del dispositivo a quien se
/// conecte (y con `su`, control root).  Por eso:
///   * exige `confirm == true`;
///   * se puede limitar a una IP de origen con `allow_from` (regla de
///     iptables en el puerto 5555);
///   * se puede desactivar en cualquier momento con `enable=false`.
pub fn remote_adb(enable: bool, confirm: bool, allow_from: &str) -> Value {
    if enable && !confirm {
        return json!({
            "ok": false,
            "enabled": false,
            "error": "confirm debe ser true: abrir ADB por TCP da control remoto del dispositivo",
            "hint": "repite la llamada con confirm=true (y usa allow_from para limitar a una IP)",
        });
    }

    let allow = allow_from.trim();
    let cmd = if enable {
        // 1) Regla de firewall (si se pidió una IP de origen).
        let fw = if allow.is_empty() {
            String::new()
        } else {
            format!(
                "iptables -I INPUT -p tcp --dport {p} -s {ip} -j ACCEPT 2>/dev/null; \
                 iptables -I INPUT -p tcp --dport {p} ! -s {ip} -j DROP 2>/dev/null;",
                p = ADB_TCP_PORT,
                ip = allow
            )
        };
        format!(
            "{fw} setprop service.adb.tcp.port {p}; stop adbd; start adbd; \
             echo ENABLED:$(getprop service.adb.tcp.port)",
            fw = fw,
            p = ADB_TCP_PORT
        )
    } else {
        format!(
            "iptables -D INPUT -p tcp --dport {p} 2>/dev/null; \
             setprop service.adb.tcp.port -1; stop adbd; start adbd; \
             echo DISABLED:$(getprop service.adb.tcp.port)",
            p = ADB_TCP_PORT
        )
    };

    let res = crate::android::root_exec(&cmd, 45_000, 4096);
    let out = res.combined();
    let ok = res.code == Some(0)
        && (out.contains("ENABLED:") || out.contains("DISABLED:"));
    let mut status = adb_status();
    if let Some(obj) = status.as_object_mut() {
        obj.insert("ok".into(), json!(ok));
        obj.insert("allow_from".into(), json!(allow));
        obj.insert("output".into(), json!(out.trim()));
    }
    status
}

// ── Tests (SIN RED) ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Índice F-Droid sintético (formato index-v1.json).
    fn fdroid_index_sintetico() -> Value {
        json!({
            "packages": [
                {
                    "packageName": "org.fdroid.fdroid",
                    "versionName": "0.108.0",
                    "versionCode": 10800,
                    "apkName": "org.fdroid.fdroid_10800.apk",
                    "hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "size": 8123456,
                    "addedDate": 1714644000000u64
                },
                {
                    "packageName": "com.example.lector",
                    "versionName": "2.1.0",
                    "versionCode": 210,
                    "apkName": "com.example.lector_210.apk",
                    "hash": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "size": 1048576,
                    "addedDate": 1714644000000u64
                }
            ]
        })
    }

    /// Índice Obtainium sintético.
    fn obtainium_index_sintetico() -> Value {
        json!([
            {
                "packageName": "org.fdroid.fdroid",
                "versionName": "0.109.0",
                "versionCode": 10900,
                "apkName": "org.fdroid.fdroid_10900.apk",
                "url": "https://obtainium.example/apk/org.fdroid.fdroid_10900.apk"
            }
        ])
    }

    fn repo_fdummy() -> RepoSpec {
        RepoSpec {
            id: "fdroid".into(),
            name: "F-Droid".into(),
            kind: "fdroid".into(),
            index_urls: vec!["https://fdlf01.f-droid.org/repo/index-v1.json".into()],
            auth: "none".into(),
            token: String::new(),
            cookie: String::new(),
            direct_download: true,
            configured: true,
            note: String::new(),
        }
    }

    fn normaliza(indice: &Value, spec: &RepoSpec, url: &str) -> Vec<Value> {
        normalize_index(spec, indice, url)
    }

    // (a) NORMALIZACIÓN ----------------------------------------------
    #[test]
    fn normaliza_el_indice_de_fdroid() {
        let spec = repo_fdummy();
        let entries = normaliza(&fdroid_index_sintetico(), &spec, &spec.index_urls[0]);
        assert_eq!(entries.len(), 2, "deben salir las 2 apps del índice");

        let e = &entries[0];
        assert_eq!(e["package"], "org.fdroid.fdroid");
        assert_eq!(e["version_name"], "0.108.0");
        assert_eq!(e["version_code"], 10800);
        assert_eq!(
            e["apk_url"],
            "https://fdlf01.f-droid.org/repo/org.fdroid.fdroid_10800.apk"
        );
        assert_eq!(
            e["sha256"],
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(e["size"], 8123456);
        assert!(e["added_date"].as_str().unwrap().starts_with("2024-05-"));
    }

    #[test]
    fn normaliza_el_indice_de_obtainium() {
        let spec = RepoSpec {
            id: "ob".into(),
            name: "Ob".into(),
            kind: "obtainium".into(),
            index_urls: vec!["https://host/index.json".into()],
            auth: "none".into(),
            token: String::new(),
            cookie: String::new(),
            direct_download: true,
            configured: true,
            note: String::new(),
        };
        let entries = normaliza(&obtainium_index_sintetico(), &spec, &spec.index_urls[0]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["package"], "org.fdroid.fdroid");
        assert_eq!(entries[0]["version_code"], 10900);
        assert_eq!(
            entries[0]["apk_url"],
            "https://obtainium.example/apk/org.fdroid.fdroid_10900.apk"
        );
        // Obtainium no publica hash: el campo queda vacío (no se inventa).
        assert_eq!(entries[0]["sha256"], "");
    }

    #[test]
    fn el_parseo_es_tolerante_a_campos_faltantes() {
        let spec = repo_fdummy();
        let indice = json!({
            "packages": [
                { "packageName": "com.app.minimo" },                     // sin versión
                { "versionName": "1.0", "versionCode": 3 },              // sin paquete -> se descarta
                { "packageName": "com.app.texto", "versionCode": "42",   // versionCode como texto
                  "hash": "no-es-un-hash" }                              // hash inválido -> ignorado
            ]
        });
        let entries = normaliza(&indice, &spec, &spec.index_urls[0]);
        assert_eq!(entries.len(), 2, "la entrada sin paquete se descarta");
        assert_eq!(entries[0]["version_code"], -1, "versión desconocida = -1");
        assert_eq!(entries[0]["apk_url"], "");
        assert_eq!(entries[1]["version_code"], 42, "el versionCode en texto se parsea");
        assert_eq!(entries[1]["sha256"], "", "un hash que no es SHA-256 se descarta");
    }

    // (b) DETECCIÓN DE ACTUALIZACIÓN ----------------------------------
    #[test]
    fn detecta_una_actualizacion_disponible() {
        let spec = repo_fdummy();
        let entries = normaliza(&fdroid_index_sintetico(), &spec, &spec.index_urls[0]);
        let installed = vec![json!({
            "package": "org.fdroid.fdroid",
            "version_name": "0.107.0",
            "version_code": 10700,
            "apk_path": "/data/app/org.fdroid.fdroid/base.apk",
            "sha256": ""
        })];
        let indexes = vec![json!({ "source": "fdroid", "packages": entries })];

        let plan = build_plan(&installed, &indexes);
        assert_eq!(plan["updatable"], 1);
        assert_eq!(plan["total"], 1);
        let u = &plan["updates"][0];
        assert_eq!(u["package"], "org.fdroid.fdroid");
        assert_eq!(u["installed_version"], "0.107.0");
        assert_eq!(u["available_version"], "0.108.0");
        assert_eq!(u["source"], "fdroid");
        assert_eq!(u["action"], "download");
        assert_eq!(u["risk"], "low");
        assert_eq!(u["size"], 8123456);
        assert!(u["sha256"].as_str().unwrap().starts_with("aaaa"));
    }

    // (c) APP SIN VERSIÓN NUEVA -> NO APARECE -------------------------
    #[test]
    fn una_app_sin_version_nueva_no_aparece_en_el_plan() {
        let spec = repo_fdummy();
        let entries = normaliza(&fdroid_index_sintetico(), &spec, &spec.index_urls[0]);
        let installed = vec![json!({
            "package": "org.fdroid.fdroid",
            "version_name": "0.108.0",
            "version_code": 10800
        })];
        let indexes = vec![json!({ "source": "fdroid", "packages": entries })];

        let plan = build_plan(&installed, &indexes);
        assert_eq!(plan["updatable"], 0);
        assert_eq!(plan["updates"].as_array().unwrap().len(), 0);
        assert_eq!(plan["blocked"], 0);
        assert_eq!(plan["total"], 1, "la app se evaluó, pero no hay nada que hacer");
    }

    // (d) DOWNGRADE -> RIESGO ALTO -----------------------------------
    #[test]
    fn un_downgrade_se_marca_con_riesgo_alto() {
        let spec = repo_fdummy();
        let entries = normaliza(&fdroid_index_sintetico(), &spec, &spec.index_urls[0]);
        // El repo ofrece 10800, el móvil tiene 10900 -> downgrade.
        let installed = vec![json!({
            "package": "org.fdroid.fdroid",
            "version_name": "0.109.0",
            "version_code": 10900
        })];
        let indexes = vec![json!({ "source": "fdroid", "packages": entries })];

        let plan = build_plan(&installed, &indexes);
        assert_eq!(plan["updates"].as_array().unwrap().len(), 1);
        let u = &plan["updates"][0];
        assert_eq!(u["risk"], "high");
        assert_eq!(u["blocked"], true);
        assert!(u["reason"].as_str().unwrap().contains("downgrade"));
        assert_eq!(plan["blocked"], 1);
        assert_eq!(plan["updatable"], 0, "un downgrade NO es una actualización accionable");
    }

    // (e) PLAN VACÍO SIN DATOS ----------------------------------------
    #[test]
    fn el_plan_vacio_sin_datos_no_rompe() {
        let plan = build_plan(&[], &[]);
        assert_eq!(plan["ok"], true);
        assert_eq!(plan["total"], 0);
        assert_eq!(plan["updatable"], 0);
        assert_eq!(plan["blocked"], 0);
        assert_eq!(plan["updates"].as_array().unwrap().len(), 0);
        assert_eq!(plan["blocked_items"].as_array().unwrap().len(), 0);

        // Inventario con basura tampoco debe romper nada.
        let basura = vec![json!({}), json!("no soy un objeto"), json!({"package": ""})];
        let plan2 = build_plan(&basura, &[]);
        assert_eq!(plan2["total"], 0);
        assert_eq!(plan2["updatable"], 0);
    }

    // Extras ----------------------------------------------------------
    #[test]
    fn gana_el_repo_con_la_version_mas_alta() {
        let spec = repo_fdummy();
        let fdroid = normaliza(&fdroid_index_sintetico(), &spec, &spec.index_urls[0]);
        let obtainium = normaliza(&obtainium_index_sintetico(), &spec, "https://host/index.json");
        let indexes = vec![
            json!({ "source": "fdroid", "packages": fdroid }),
            json!({ "source": "obtainium", "packages": obtainium }),
        ];
        let installed = vec![json!({
            "package": "org.fdroid.fdroid",
            "version_name": "0.100.0",
            "version_code": 10000
        })];
        let plan = build_plan(&installed, &indexes);
        assert_eq!(plan["updatable"], 1);
        let u = &plan["updates"][0];
        assert_eq!(u["source"], "obtainium", "gana el repo con el versionCode más alto");
        assert_eq!(u["available_version_code"], 10900);
    }

    #[test]
    fn una_app_sin_hash_nunca_es_de_riesgo_bajo() {
        let spec = repo_fdummy();
        let entries = normaliza(&obtainium_index_sintetico(), &spec, "https://host/index.json");
        let installed = vec![json!({
            "package": "org.fdroid.fdroid",
            "version_name": "0.108.0",
            "version_code": 10800
        })];
        let indexes = vec![json!({ "source": "obtainium", "packages": entries })];
        let plan = build_plan(&installed, &indexes);
        assert_eq!(plan["updates"][0]["risk"], "medium");
    }

    #[test]
    fn el_catalogo_de_repos_es_consistente() {
        let catalogo = repos();
        let lista = catalogo["repos"].as_array().unwrap();
        assert!(!lista.is_empty());
        let fdroid = lista.iter().find(|r| r["id"] == "fdroid").expect("F-Droid debe estar");
        assert_eq!(fdroid["kind"], "fdroid");
        assert_eq!(fdroid["auth"], "none");
        assert_eq!(fdroid["direct_download"], true);
        assert!(fdroid["index_url"].as_str().unwrap().contains("index-v1.json"));

        // Play: sin endpoint inventado y explicado.
        let play = lista.iter().find(|r| r["id"] == "play").expect("Play debe estar");
        assert_eq!(play["kind"], "play");
        assert_eq!(play["index_url"], "");
        assert_eq!(play["configured"], false);
        assert_eq!(play["direct_download"], false);
        assert!(play["note"].as_str().unwrap().contains("NO ofrece API pública"));
    }

    #[test]
    fn el_catalogo_declara_los_fuentes_de_play_sin_api() {
        // El repo "play" debe explicar el porqué, no inventar un endpoint.
        let lista = all_repos();
        let play = lista.iter().find(|r| r.id == "play").unwrap();
        assert!(play.index_urls.is_empty());
        assert!(play.note.to_lowercase().contains("api pública"));
        // Y no debe aparecer entre las fuentes por defecto.
        assert!(!default_sources().contains(&"play".to_string()));
    }

    #[test]
    fn fetch_index_de_un_repo_desconocido_no_rompe() {
        let r = fetch_index("no-existe-este-repo");
        assert_eq!(r["ok"], false);
        assert_eq!(r["count"], 0);
        assert_eq!(r["packages"].as_array().unwrap().len(), 0);
        assert!(r["error"].as_str().unwrap().contains("desconocida"));
    }

    #[test]
    fn aplicar_sin_confirmacion_no_hace_nada() {
        let r = apply("com.example.lector", "/no/existe.apk", false);
        assert_eq!(r["ok"], false);
        assert_eq!(r["installed"], false);
        assert!(r["error"].as_str().unwrap().contains("confirm"));
    }

    #[test]
    fn un_hash_incorrecto_borra_el_fichero() {
        let dir = std::env::temp_dir().join(format!("md-repo-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let tmp = dir.join("x.apk.part");
        let final_path = dir.join("x.apk");
        std::fs::write(&tmp, b"contenido falso").unwrap();

        let r = finalize_download(&tmp, &final_path, &"0".repeat(64), 13);
        assert_eq!(r["ok"], false);
        assert_eq!(r["verified"], false);
        assert!(r["error"].as_str().unwrap().contains("SHA-256"));
        assert!(!tmp.exists(), "el fichero con hash incorrecto debe borrarse");
        assert!(!final_path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_hash_correcto_conserva_el_fichero() {
        let dir = std::env::temp_dir().join(format!("md-repo-test-ok-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let tmp = dir.join("y.apk.part");
        let final_path = dir.join("y.apk");
        let contenido = b"contenido del apk";
        std::fs::write(&tmp, contenido).unwrap();

        // SHA-256 real del contenido.
        let (digest, _size) = sha256_file(&tmp).unwrap();
        let r = finalize_download(&tmp, &final_path, &digest, contenido.len() as u64);
        assert_eq!(r["ok"], true);
        assert_eq!(r["verified"], true);
        assert_eq!(r["sha256"], digest.as_str());
        assert!(final_path.exists(), "el APK verificado debe quedarse en su sitio");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_inventario_local_se_parsea_desde_dumpsys() {
        let texto = "\
  Package [org.fdroid.fdroid] (1a2b3c):
    userId=10123
    codePath=/data/app/~~abc==/org.fdroid.fdroid-xyz
    versionCode=10800 minSdk=23 targetSdk=34
    versionName=0.108.0
  Package [com.ejemplo.app] (4d5e6f):
    codePath=/data/app/~~def==/com.ejemplo.app-uvw
    versionCode=7 minSdk=21 targetSdk=33
    versionName=0.7
";
        let apps = parse_dumpsys_packages(texto);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0]["package"], "org.fdroid.fdroid");
        assert_eq!(apps[0]["version_code"], 10800);
        assert_eq!(apps[0]["version_name"], "0.108.0");
        assert_eq!(apps[1]["package"], "com.ejemplo.app");
        assert_eq!(apps[1]["version_code"], 7);
    }

    #[test]
    fn las_fechas_en_epoch_se_convierten_bien() {
        assert_eq!(epoch_ms_to_date(0), "1970-01-01 00:00:00Z");
        assert_eq!(epoch_ms_to_date(1_714_644_000_000), "2024-05-02 10:00:00Z");
        assert!(epoch_to_iso(1_714_644_000).starts_with("2024-05-02T10:00:00"));
    }

    // ── Despliegue y ADB remoto ───────────────────────────────────────
    #[test]
    fn el_sondeo_de_salud_se_interpreta_bien() {
        let (ok, code) = parse_health_output("ruido\nHEALTH:200\nmas ruido");
        assert!(ok);
        assert_eq!(code, "200");

        let (ok, code) = parse_health_output("HEALTH:000");
        assert!(!ok);
        assert_eq!(code, "000");

        // Si el sondeo no llegó a Prints (p.ej. sin root), es "no responde".
        let (ok, code) = parse_health_output("sin salida de salud");
        assert!(!ok);
        assert_eq!(code, "000");
    }

    #[test]
    fn desplegar_sin_confirmacion_no_toca_nada() {
        let r = deploy_binary("megadaemon-core", "/no/existe", "", false);
        assert_eq!(r["ok"], false);
        assert_eq!(r["installed"], false);
        assert!(r["error"].as_str().unwrap().contains("confirm"));
    }

    #[test]
    fn desplegar_un_fichero_inexistente_falla() {
        let r = deploy_binary("megadaemon-core", "/no/existe/megadaemon-core", "", true);
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().contains("no existe"));
    }

    #[test]
    fn adb_remoto_exige_confirmacion() {
        let r = remote_adb(true, false, "");
        assert_eq!(r["ok"], false);
        assert_eq!(r["enabled"], false);
        assert!(r["error"].as_str().unwrap().contains("confirm"));
    }
}
