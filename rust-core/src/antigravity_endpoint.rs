//! Puente de endpoint: hacer que el language server (LS) de Antigravity
//! hable **siempre** con el MegaDaemon canónico, y que el selector de la
//! GUI ofrezca el modelo `megadaemon`.
//!
//! # Qué está medido (móvil S21, `com.agy` 2.11.0, 2026-11-22)
//!
//! 1. `runtime/bin/liblanguage_server.so` (ELF aarch64, 181.358.800 B) contiene
//!    **una** vez la cadena `https://daily-cloudcode-pa.googleapis.com`
//!    (42 B) → es el endpoint de Cloud Code. El LS llama
//!    `<base>/v1internal:generateContent` (MEDIDO: strings
//!    `/v1internal:generateContent` y `/v1internal:streamGenerateContent`).
//! 2. `web/generated/main.js` contiene la condición del selector de modelos:
//!    `osName==="linux"&&k.isGoogleInternal` ⇒ en Android los modelos
//!    personalizados se marcan `disabled=true` y **la GUI no los ofrece**
//!    (aunque el BYOM esté registrado en `antigravity_state.pbtxt`).
//!
//! # Referencia externa
//!
//! Proyecto `vahapogut/antigravity-add-model` (Apache-2.0), encontrado en el
//! móvil en `/data/user/0/com.agy/files/antigravity-add-model`. Hace lo mismo
//! que aquí pero con **su** proxy:
//!   - `scripts/language-server-binary-patch.mjs`: sustituye la URL de Cloud
//!     Code por `http://localhost:50999/v1internal/xxxxxxx` (42 B ⇒ el
//!     reemplazo **conserva la longitud**, por eso vale para cualquier SO).
//!   - `scripts/ide.mjs`: el IDE lee el ajuste oculto
//!     `jetski.cloudCodeUrl` y lo inyecta como `CLOUD_CODE_URL` (o
//!     `AGY_LLM_GATEWAY_URL`, que **sustituye la lista de modelos** nativa).
//!
//! Aquí no se reimplementa ese proxy: se usa el **gateway canónico** del
//! MegaDaemon (dialecto Gemini ya implementado en `llm_gateway.rs`), de modo
//! que no hay un tercer motor de inferencia.
//!
//! # Reglas
//!
//! * Todo parche es **reversible**: copia `.bak-megadaemon` antes de tocar.
//! * El parche del binario **conserva la longitud** ⇒ no cambia el layout
//!   del ELF y el sha256 del manifiesto se actualiza después.
//! * Nada se escribe sin `confirm=true` (MCP) / `--confirm` (CLI).

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Endpoint de Cloud Code presente en el LS de Antigravity (42 bytes).
pub const CLOUCODE_URL: &str = "https://daily-cloudcode-pa.googleapis.com";

/// Condición que la GUI usa para habilitar los modelos personalizados.
pub const GATE_CONDICION: &str = "osName===\"linux\"&&k.isGoogleInternal";

/// Puerto por defecto del gateway (cockpit) del MegaDaemon en el móvil.
pub const PUERTO_GATEWAY: u16 = 9090;

/// Raíz de los datos de la app en el móvil.
pub const RAIZ_APP: &str = "/data/user/0/com.agy/files";

fn sha256_hex(datos: &[u8]) -> String {
    crate::self_update::sha256_hex(datos)
}

fn sha256_de(p: &Path) -> Option<String> {
    let datos = std::fs::read(p).ok()?;
    Some(sha256_hex(&datos))
}

/// Candidatas del binario del language server.
///
/// MEDIDO (2026-11-23): la ruta estaba HARDCODEADA y Antigravity, al
/// actualizarse, movió su runtime: el `.so` ya no está en
/// `runtime/bin/liblanguage_server.so` sino en
/// `standalone_runtime/bin/language_server`, y en `agy_standalone/bin` hay
/// un **symlink roto** al path viejo.  Con la ruta fija, la
/// instrumentación fallaba con «No such file or directory» y el usuario
/// veía que «no pasaba nada».
const CANDIDATAS_LS: &[&str] = &[
    "runtime/bin/liblanguage_server.so",
    "standalone_runtime/bin/liblanguage_server.so",
    "standalone_runtime/bin/language_server",
    "agy_standalone/bin/liblanguage_server.so",
    "agy_standalone/bin/language_server",
];

/// Ruta del binario del language server.
///
/// Se prueban las candidatas conocidas y, si ninguna existe, se **descubre**
/// en el árbol de la app (con presupuesto acotado y cacheado: el árbol de
/// la app es enorme).
pub fn ruta_ls() -> PathBuf {
    for rel in CANDIDATAS_LS {
        let p = PathBuf::from(RAIZ_APP).join(rel);
        if p.exists() {
            return p;
        }
    }
    descubrir_ls().unwrap_or_else(|| PathBuf::from(RAIZ_APP).join(CANDIDATAS_LS[0]))
}

/// Cache del descubrimiento (la búsqueda en el árbol de la app no es gratis).
static LS_DESCUBIERTO: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Busca el language server en el árbol de la app (profundidad acotada).
fn descubrir_ls() -> Option<PathBuf> {
    if let Some(v) = LS_DESCUBIERTO.get() {
        return v.clone();
    }
    let mut mejor: Option<(u64, PathBuf)> = None;
    let base = PathBuf::from(RAIZ_APP);
    let nombres = ["liblanguage_server.so", "language_server"];
    fn visitar(dir: &Path, prof: usize, nombres: &[&str], mejor: &mut Option<(u64, PathBuf)>) {
        if prof > 3 {
            return;
        }
        let Ok(entradas) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entradas.flatten() {
            let p = e.path();
            let nombre = e.file_name().to_string_lossy().into_owned();
            // Se ignoran los directorios gigantes que no tienen nada que ver.
            if nombre == "google-cloud-sdk"
                || nombre == "gpui-mobile"
                || nombre == "node_modules"
                || nombre.starts_with('.')
            {
                continue;
            }
            if p.is_dir() {
                visitar(&p, prof + 1, nombres, mejor);
                continue;
            }
            if !nombres.contains(&nombre.as_str()) {
                continue;
            }
            let meta = match e.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let tam = meta.len();
            // El language server real pesa ~180 MB: un symlink o un stub no
            // sirve.  A igualdad de tamano, gana el mas reciente.
            let fecha = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let clave = tam.saturating_mul(1000).saturating_add(fecha);
            if tam > 50 * 1024 * 1024
                && mejor.as_ref().map(|(c, _)| clave > *c).unwrap_or(true)
            {
                *mejor = Some((clave, p.clone()));
            }
        }
    }
    visitar(&base, 0, &nombres, &mut mejor);
    let r = mejor.map(|(_, p)| p);
    let _ = LS_DESCUBIERTO.set(r.clone());
    r
}

/// Ruta del manifiesto del runtime (sha256 por fichero).
///
/// También con candidatas: el manifiesto va junto al runtime y este se
/// movió al actualizarse la app (medido el 2026-11-23).
pub fn ruta_manifest() -> PathBuf {
    for rel in [
        "runtime/manifest.json",
        "standalone_runtime/manifest.json",
        "agy_standalone/manifest.json",
    ] {
        let p = PathBuf::from(RAIZ_APP).join(rel);
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(RAIZ_APP).join("runtime/manifest.json")
}

/// Ruta del bundle web que decide si los modelos personalizados se ofrecen.
pub fn ruta_gate() -> PathBuf {
    PathBuf::from(RAIZ_APP).join("web/generated/main.js")
}

/// Longitud del secreto local del agente (hex).
///
/// MEDIDO (2026-11-23): la URL original (`https://daily-cloudcode-pa.googleapis.com`)
/// mide **41** bytes, no 42 ⇒ con el prefijo "/mda/" (5) el secreto tiene que
/// medir 15 (21 + 5 + 15 = 41) o la URL no cabe y el parche se rechaza.
pub const LARGO_SECRETO: usize = 15;

/// Prefijo de la ruta que usa el **language server** para hablar con el
/// gateway.
///
/// MEDIDO (2026-11-23): con el prefijo anterior (`/v1internal/xxxx…`) las
/// peticiones del LS al gateway acababan en **403 «falta la cabecera
/// X-Seere-Auth-Token»**: el LS no lleva nuestro token de sesión (no puede
/// saberlo) y el gate lo rechazaba antes de llegar al shim ⇒ la app no
/// podía usar el modelo de MegaDaemon.
///
/// Ahora el prefijo lleva un **secreto local**: `/mda/<16 hex>`.  El
/// daemon valida el secreto, da la petición por autenticada y quita el
/// prefijo para que el shim la enrute como una petición Gemini normal.
/// El secreto vive en `<state>/agy_proxy.secret` (0600) y en el binario
/// de la app (que sólo leen la app y root) ⇒ el túnel público no puede
/// adivinarlo.
pub const PREFIJO_AGENTE: &str = "/mda";

/// Secreto local del proxy del agente (15 hex, 0600, estable).
///
/// Se crea la primera vez y no cambia: si cambiara, el binario parcheado
/// dejaría de validar (y habría que re-patchear los 181 MB).
pub fn secreto_agente() -> String {
    // Cache: se consulta en CADA petición del agente (el gate lo llama) y
    // el fichero es de 16 bytes: no hace falta leerlo cada vez.
    //
    // MEDIDO (2026-11-23): con `get` + `set` dos hilos podían inicializar a
    // la vez y obtener **secretos DISTINTOS** (el plan B usa el reloj) ⇒ la
    // URL parcheada y el secreto del gate no cuadraban (y el fichero
    // acababa con otro valor).  `get_or_init` garantiza una sola
    // inicialización.
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE.get_or_init(secreto_agente_real).clone()
}

fn secreto_agente_real() -> String {
    let dir = crate::cockpit_server::state_dir();
    let ruta = std::path::PathBuf::from(&dir).join("agy_proxy.secret");
    if let Ok(s) = std::fs::read_to_string(&ruta) {
        let t = s.trim().to_string();
        if t.len() == LARGO_SECRETO && t.chars().all(|c| c.is_ascii_hexdigit()) {
            return t;
        }
    }
    // 8 bytes de /dev/urandom en hexadecimal (16 caracteres).
    let mut buf = [0u8; 8];
    let nuevo = match std::fs::File::open("/dev/urandom") {
        Ok(mut f) => {
            use std::io::Read;
            f.read_exact(&mut buf).is_ok()
        }
        Err(_) => false,
    };
    let hex = if nuevo {
        buf.iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
            .chars()
            .take(LARGO_SECRETO)
            .collect::<String>()
    } else {
        // Plan B (sin /dev/urandom): el commit de la build (estable entre
        // reinicios ⇒ no obliga a re-patchear los 181 MB en cada arranque).
        let sha = crate::self_update::build_commit();
        let t = sha
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .take(LARGO_SECRETO)
            .collect::<String>();
        if t.len() == LARGO_SECRETO {
            t
        } else {
            let n = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            format!("{:0width$x}", n ^ (std::process::id() as u64).wrapping_mul(0x9E3779B97F4A7C15), width = LARGO_SECRETO)
        }
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&ruta, format!("{hex}\n"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&ruta, std::fs::Permissions::from_mode(0o600));
    }
    hex
}

/// URL destino para el LS, **de la misma longitud** que [`CLOUCODE_URL`].
///
/// `http://127.0.0.1:<puerto>/mda/<15 hex>` → 41 bytes exactos con un
/// puerto de 4 dígitos (21 + 5 + 15), los mismos que la URL original.
pub fn url_destino(puerto: u16) -> String {
    let base = format!("http://127.0.0.1:{puerto}{PREFIJO_AGENTE}/");
    let hex = secreto_agente();
    let mut s = format!("{base}{hex}");
    if s.len() > CLOUCODE_URL.len() {
        s.truncate(CLOUCODE_URL.len());
    } else {
        s.push_str(&"x".repeat(CLOUCODE_URL.len() - s.len()));
    }
    s
}

/// ¿Esta ruta viene del language server instrumentado (secreto correcto)?
///
/// Si es así, la petición está autenticada por el secreto y hay que
/// **quitar el prefijo** antes de enrutar.
pub fn quitar_prefijo_agente(path: &str) -> Option<String> {
    let p = path.trim();
    let pref = format!("{PREFIJO_AGENTE}/");
    let resto = p.strip_prefix(pref.as_str())?;
    // `get(..16)` devuelve None si no hay 16 bytes o si el 16 no cae en un
    // borde de carácter (nada de `split_at`, que entra en pánico).
    let secreto = resto.get(..LARGO_SECRETO)?;
    let cola = &resto[LARGO_SECRETO..];
    if !secreto.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    if secreto != secreto_agente() {
        return None;
    }
    if cola.is_empty() {
        return Some("/".to_string());
    }
    // Relleno de 'x' (si el URL quedó corto: el LS añade su ruta detrás).
    let cola = cola.trim_start_matches('x');
    if cola.is_empty() {
        return Some("/".to_string());
    }
    if cola.starts_with('/') || cola.starts_with(':') {
        return Some(cola.to_string());
    }
    None
}

/// ¿La URL destino conserva la longitud exacta de la original?
pub fn url_destino_valido(puerto: u16) -> bool {
    let u = url_destino(puerto);
    u.len() == CLOUCODE_URL.len() && u.contains(&secreto_agente())
}

/// Catálogo de modelos en formato Gemini (`GET /v1beta/models`).
///
/// Lo consume el LS de Antigravity cuando su endpoint apunta al gateway
/// (modo "LLM gateway"): la lista **sustituye** a la nativa de Google, así
/// que aquí sale el modelo `megadaemon` y los proveedores del vault.
pub fn catalogo_gemini() -> Value {
    let vault = crate::providers::load_vault();
    let mut models: Vec<Value> = Vec::new();

    models.push(json!({
        "name": "models/megadaemon",
        "version": "1.0",
        "displayName": "MegaDaemon",
        "description": "Gateway canónico del MegaDaemon: enruta a los proveedores del vault con fallback real.",
        "inputTokenLimit": 1_000_000,
        "outputTokenLimit": 65_536,
        "supportedGenerationMethods": ["generateContent", "countTokens", "streamGenerateContent"],
        "temperature": 0.7,
        "topP": 0.9,
        "topK": 40,
    }));

    for p in vault.providers.iter().filter(|p| p.enabled) {
        models.push(json!({
            "name": format!("models/{}", p.id),
            "version": "1.0",
            "displayName": if p.name.trim().is_empty() { p.id.clone() } else { p.name.clone() },
            "description": format!("Proveedor del MegaDaemon: {} ({}).", p.vendor, p.model),
            "inputTokenLimit": 200_000,
            "outputTokenLimit": 32_768,
            "supportedGenerationMethods": ["generateContent", "countTokens", "streamGenerateContent"],
            "temperature": 0.7,
            "topP": 0.9,
            "topK": 40,
        }));
    }

    json!({ "models": models })
}

/// Estado (sólo lectura) de las dos palancas.
pub fn estado() -> Value {
    let ls = ruta_ls();
    let gate = ruta_gate();
    let manifest = ruta_manifest();

    let (occ, parcheado, sha) = match std::fs::read(&ls) {
        Ok(d) => {
            let original = CLOUCODE_URL.as_bytes();
            let occ = d.windows(original.len()).filter(|w| *w == original).count();
            let destino = url_destino(PUERTO_GATEWAY);
            let parcheado = d.windows(destino.len()).any(|w| w == destino.as_bytes());
            (occ, parcheado, Some(sha256_hex(&d)))
        }
        Err(_) => (0, false, None),
    };

    let gate_info = match std::fs::read_to_string(&gate) {
        Ok(t) => {
            let occ = t.matches(GATE_CONDICION).count();
            json!({
                "ruta": gate.to_string_lossy(),
                "ocurrencias": occ,
                "parcheado": occ == 0 && t.contains("isGoogleInternal") == false,
                "nota": "parcheado = la condición `osName===\"linux\"&&k.isGoogleInternal` ya no aparece",
            })
        }
        Err(e) => json!({ "ruta": gate.to_string_lossy(), "error": e.to_string() }),
    };

    let manifest_sha = std::fs::read_to_string(&manifest)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| {
            v.get("files")
                .and_then(|f| f.get("bin/liblanguage_server.so"))
                .and_then(|s| s.as_str().map(|s| s.to_string()))
        });

    json!({
        "endpoint": {
            "ruta": ls.to_string_lossy(),
            "ocurrencias_url_original": occ,
            "parcheado": parcheado,
            "url_original": CLOUCODE_URL,
            "url_destino": url_destino(PUERTO_GATEWAY),
            "longitud_conservada": url_destino_valido(PUERTO_GATEWAY),
            "sha256": sha,
            "sha256_manifesto": manifest_sha,
            "sha256_coincide_manifesto": match (&sha, &manifest_sha) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            },
        },
        "gate": gate_info,
        "ok": true,
    })
}

/// Plan del parche del binario (no escribe nada).
pub fn plan_endpoint(puerto: u16) -> Value {
    let url = url_destino(puerto);
    let datos = match std::fs::read(ruta_ls()) {
        Ok(d) => d,
        Err(e) => {
            return json!({ "ok": false, "error": e.to_string(), "ruta": ruta_ls().to_string_lossy() });
        }
    };
    let occ = datos.windows(CLOUCODE_URL.len()).filter(|w| *w == CLOUCODE_URL.as_bytes()).count();
    json!({
        "ok": true,
        "ruta": ruta_ls().to_string_lossy(),
        "ocurrencias": occ,
        "de": CLOUCODE_URL,
        "a": url,
        "longitud_original": CLOUCODE_URL.len(),
        "longitud_destino": url.len(),
        "conserva_longitud": url.len() == CLOUCODE_URL.len(),
        "respaldo": format!("{}.bak-megadaemon", ruta_ls().to_string_lossy()),
        "nota": "el manifiesto (runtime/manifest.json) se actualiza con el sha256 nuevo",
    })
}

/// Aplica el parche del binario: copia de seguridad + sustitución + manifiesto.
pub fn aplicar_endpoint(puerto: u16, confirmar: bool) -> Result<Value, String> {
    if !confirmar {
        return Err("confirm_required".to_string());
    }
    if !url_destino_valido(puerto) {
        return Err(format!(
            "url_destino_invalida: {} bytes (se necesitan {})",
            url_destino(puerto).len(),
            CLOUCODE_URL.len()
        ));
    }
    let ruta = ruta_ls();
    let respaldo = ruta.with_extension("so.bak-megadaemon");
    let mut datos = std::fs::read(&ruta).map_err(|e| e.to_string())?;
    let destino = url_destino(puerto);
    let antes = CLOUCODE_URL.as_bytes();
    let despues = destino.as_bytes();

    let mut pos = 0usize;
    let mut cambios = 0usize;
    while pos + antes.len() <= datos.len() {
        if datos[pos..pos + antes.len()] == *antes {
            datos[pos..pos + antes.len()].copy_from_slice(despues);
            cambios += 1;
            pos += antes.len();
        } else {
            pos += 1;
        }
    }
    if cambios == 0 {
        return Err("sin_cambios: la URL original no aparece (¿ya parcheado?)".to_string());
    }

    if !respaldo.exists() {
        std::fs::copy(&ruta, &respaldo).map_err(|e| format!("respaldo: {e}"))?;
        // MEDIDO (2026-11-23): la copia la crea ROOT ⇒ si queda dentro del
        // runtime de la app, la app no lo puede borrar al desempaquetar y
        // NO ARRANCA.  Se devuelve al dueño de la app.
        crate::antigravity_clone::devolver_al_dueno_de_la_app(&respaldo);
    }
    std::fs::write(&ruta, &datos).map_err(|e| format!("escritura: {e}"))?;

    let sha = sha256_hex(&datos);
    let actualizado = actualizar_manifesto(&sha);

    Ok(json!({
        "ok": true,
        "cambios": cambios,
        "ruta": ruta.to_string_lossy(),
        "respaldo": respaldo.to_string_lossy(),
        "sha256_nuevo": sha,
        "manifesto_actualizado": actualizado,
        "url_destino": destino,
    }))
}

/// Actualiza el sha256 del LS en `runtime/manifest.json` (expuesto para
/// que el módulo del "falso" (`antigravity_fake_so`) reutilice ESTA
/// función y no duplique la lógica del manifiesto).
pub fn actualizar_manifesto_publico(sha: &str) -> bool {
    actualizar_manifesto(sha)
}

/// Actualiza el sha256 del LS en el manifiesto del runtime.
///
/// MEDIDO (2026-11-23): el manifiesto declara `bin/liblanguage_server.so`,
/// pero tras actualizarse la app el fichero se llama `language_server` y
/// vive en otro directorio.  Si el manifiesto no cuadra, la app **restaura
/// su runtime** y la instrumentación se pierde (y el ciclo vuelve a
/// aplicarla: un bucle).
///
/// Por eso se actualizan TODAS las claves que hablen de «language_server»,
/// además de la del nombre real del fichero.
fn actualizar_manifesto(sha: &str) -> bool {
    let nombre_real = ruta_ls()
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "liblanguage_server.so".to_string());
    let mut alguno = false;
    for ruta in manifiestos() {
        let texto = match std::fs::read_to_string(&ruta) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let mut v: Value = match serde_json::from_str(&texto) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let ok = v
            .get_mut("files")
            .and_then(|f| f.as_object_mut())
            .map(|f| {
                // 1) la clave del nombre real del fichero
                f.insert(format!("bin/{nombre_real}"), Value::String(sha.to_string()));
                // 2) cualquier clave que ya hable del language server
                let claves: Vec<String> = f
                    .keys()
                    .filter(|k| k.contains("language_server"))
                    .cloned()
                    .collect();
                for k in claves {
                    f.insert(k, Value::String(sha.to_string()));
                }
                true
            })
            .unwrap_or(false);
        if ok {
            if let Ok(s) = serde_json::to_string_pretty(&v) {
                let _ = std::fs::write(&ruta, s);
                alguno = true;
            }
        }
    }
    alguno
}

/// Todos los manifiestos de runtime que existen (el de la app actual puede
/// estar en más de un sitio tras actualizarse).
fn manifiestos() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for rel in [
        "runtime/manifest.json",
        "standalone_runtime/manifest.json",
        "agy_standalone/manifest.json",
    ] {
        let p = PathBuf::from(RAIZ_APP).join(rel);
        if p.exists() {
            out.push(p);
        }
    }
    if out.is_empty() {
        out.push(ruta_manifest());
    }
    out
}

/// Plan del parche del gate de la GUI (no escribe nada).
pub fn plan_gate() -> Value {
    let ruta = ruta_gate();
    match std::fs::read_to_string(&ruta) {
        Ok(t) => {
            let occ = t.matches(GATE_CONDICION).count();
            json!({
                "ok": true,
                "ruta": ruta.to_string_lossy(),
                "ocurrencias": occ,
                "de": GATE_CONDICION,
                "a": "!0",
                "nota": "`!0` es true en JS: habilita los modelos personalizados en cualquier SO",
                "riesgo": "bundle web regenerado por la app => puede perder el parche",
            })
        }
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

/// Aplica el parche del gate de la GUI (con respaldo).
pub fn aplicar_gate(confirmar: bool) -> Result<Value, String> {
    if !confirmar {
        return Err("confirm_required".to_string());
    }
    let ruta = ruta_gate();
    let respaldo = ruta.with_extension("js.bak-megadaemon");
    let texto = std::fs::read_to_string(&ruta).map_err(|e| e.to_string())?;
    let occ = texto.matches(GATE_CONDICION).count();
    if occ == 0 {
        return Err("sin_cambios: la condición del gate no aparece".to_string());
    }
    let nuevo = texto.replace(GATE_CONDICION, "!0");
    if !respaldo.exists() {
        std::fs::copy(&ruta, &respaldo).map_err(|e| format!("respaldo: {e}"))?;
        crate::antigravity_clone::devolver_al_dueno_de_la_app(&respaldo);
    }
    std::fs::write(&ruta, nuevo).map_err(|e| format!("escritura: {e}"))?;
    Ok(json!({
        "ok": true,
        "cambios": occ,
        "ruta": ruta.to_string_lossy(),
        "respaldo": respaldo.to_string_lossy(),
    }))
}

/// Restaura los dos ficheros desde sus respaldos.
pub fn restaurar(confirmar: bool) -> Result<Value, String> {
    if !confirmar {
        return Err("confirm_required".to_string());
    }
    let mut restaurados = Vec::new();
    for (orig, bak) in [
        (ruta_ls(), ruta_ls().with_extension("so.bak-megadaemon")),
        (ruta_gate(), ruta_gate().with_extension("js.bak-megadaemon")),
    ] {
        if bak.exists() {
            std::fs::copy(&bak, &orig).map_err(|e| format!("restaurar {}: {e}", orig.to_string_lossy()))?;
            restaurados.push(orig.to_string_lossy().to_string());
        }
    }
    // Tras restaurar, el sha256 vuelve a ser el original: hay que reponerlo
    // en el manifiesto o la app lo consideraría alterado.
    if let Some(sha) = sha256_de(&ruta_ls()) {
        let _ = actualizar_manifesto(&sha);
    }
    Ok(json!({ "ok": true, "restaurados": restaurados }))
}

/// CLI + MCP: `antigravity_endpoint`.
pub fn dispatch(name: &str, args: &Value) -> Option<Value> {
    if name != "antigravity_endpoint" {
        return None;
    }
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let accion = {
        let a = s("action");
        if a.trim().is_empty() {
            "estado".to_string()
        } else {
            a
        }
    };
    let confirm = args.get("confirm").and_then(|v| v.as_bool()).unwrap_or(false);
    let puerto: u16 = args
        .get("puerto")
        .and_then(|v| v.as_u64())
        .unwrap_or(PUERTO_GATEWAY as u64) as u16;

    let v = match accion.as_str() {
        "estado" => estado(),
        "plan" => plan_endpoint(puerto),
        "plan_gate" => plan_gate(),
        "catalogo" => catalogo_gemini(),
        "aplicar" => match aplicar_endpoint(puerto, confirm) {
            Ok(v) => v,
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "aplicar_gate" => match aplicar_gate(confirm) {
            Ok(v) => v,
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "restaurar" => match restaurar(confirm) {
            Ok(v) => v,
            Err(e) => json!({ "ok": false, "error": e }),
        },
        _ => json!({
            "ok": false,
            "error": "accion_desconocida",
            "acciones": ["estado", "plan", "plan_gate", "catalogo", "aplicar", "aplicar_gate", "restaurar"],
        }),
    };
    Some(json!({ "tool": "antigravity_endpoint", "accion": accion, "resultado": v }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_destino_conserva_longitud() {
        // El prefijo del agente con su secreto ocupa 21 bytes: con un puerto
        // de 4 dígitos la URL encaja EXACTA (42 bytes, como la original). Con
        // 5 dígitos NO cabe el secreto ⇒ `url_destino_valido` lo dice (y el
        // parche se niega, en vez de escribir una URL rota en el binario).
        for p in [9090u16, 8080, 443] {
            assert_eq!(url_destino(p).len(), CLOUCODE_URL.len(), "puerto {p}");
            assert!(url_destino_valido(p), "puerto {p}");
        }
        assert!(!url_destino_valido(50999), "con 5 dígitos no cabe el secreto");
    }

    #[test]
    fn la_url_del_agente_cabe_exacta_como_la_original() {
        // MEDIDO (2026-11-23): la URL original mide 41 bytes; con el
        // prefijo y el secreto hay que llegar EXACTAMENTE a 41 o el parche
        // se rechaza (el binario es ELF: cambiar la longitud lo rompe).
        let u = url_destino(9090);
        assert_eq!(u.len(), CLOUCODE_URL.len(), "{}", u);
        assert_eq!(u.len(), 41);
        assert!(url_destino_valido(9090));
    }

    #[test]
    fn url_destino_apunta_al_gateway() {
        let u = url_destino(9090);
        assert!(u.starts_with("http://127.0.0.1:9090/mda/"), "{}", u);
        assert!(u.contains(&secreto_agente()), "la URL debe llevar el secreto");
    }

    /// MEDIDO (2026-11-23): el LS llama al gateway con el prefijo del
    /// agente; si el secreto no cuadra, la petición NO está autenticada.
    #[test]
    fn el_prefijo_del_agente_autentica_y_se_quita() {
        let sec = secreto_agente();
        let ruta = format!("/mda/{sec}/v1internal:generateContent");
        assert_eq!(
            quitar_prefijo_agente(&ruta),
            Some("/v1internal:generateContent".to_string())
        );
        // Con el relleno de 'x' (puerto de menos dígitos) también vale.
        let ruta_x = format!("/mda/{sec}x/v1internal:streamGenerateContent");
        assert_eq!(
            quitar_prefijo_agente(&ruta_x),
            Some("/v1internal:streamGenerateContent".to_string())
        );
        // Un secreto equivocado NO autentica.
        let falsa = if sec == "0123456789abcde" {
            "0123456789abcde0".to_string()
        } else {
            "0123456789abcde".to_string()
        };
        assert!(quitar_prefijo_agente(&format!("/mda/{falsa}/v1internal:x")).is_none());
        // Y una ruta normal, tampoco.
        assert!(quitar_prefijo_agente("/api/v2/dashboard").is_none());
    }

    #[test]
    fn catalogo_incluye_megadaemon() {
        let c = catalogo_gemini();
        let nombres: Vec<String> = c["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["name"].as_str().unwrap_or("").to_string())
            .collect();
        assert!(nombres.contains(&"models/megadaemon".to_string()));
    }

    #[test]
    fn dispatch_rechaza_otros_nombres() {
        assert!(dispatch("otro", &json!({})).is_none());
    }
}
