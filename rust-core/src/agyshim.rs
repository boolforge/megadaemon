//! SHIM entre Antigravity y el mundo.
//!
//! QUE PIDE EL USUARIO (2026-11-22): *"instrumentalizar MegaDaemon para
//! poner un shim entre ese binario que hace que MegaDaemon vea TODO lo que
//! hay dentro, intercepte y mienta el resto, y recoja informacion para
//! decompilar y deofuscar mejor"*.
//!
//! QUE HACE ESTE MODULO (todo local, sin dependencias externas):
//!
//! 1. **VEER**: cada petición que el *language server* de Antigravity hace
//!    (al gateway de Google o al propio daemon) pasa por el shim, que
//!    registra método, ruta, modelo pedido, tamaño, herramientas y la
//!    respuesta ⇒ un NDJSON auditable + estadísticas en memoria.
//! 2. **INTERCEPTAR**: reglas que disparan por condición (ruta, modelo,
//!    cuerpo) y que pueden (a) dejar pasar, (b) **sustituir** el cuerpo,
//!    (c) **inyectar** un campo JSON, (d) **bloquear** con un error propio.
//! 3. **MENTIR**: el shim es el punto donde la respuesta se puede
//!    **reescribir** antes de que la GUI la vea.  Es la palanca para que
//!    Antigravity crea lo que MegaDaemon quiere que crea (catálogo de
//!    modelos, tools, feature flags) ⇒ es la asimilación, con control.
//! 4. **RECOGER PARA EL RE**: el tráfico capturado alimenta al motor de RE
//!    (`re_engine`): los endpoints, los nombres de modelo y los payloads
//!    son material para deofuscar/decompilar.
//!
//! SEGURIDAD: el shim **no** reenvía a ningún sitio raro por defecto: el
//! upstream se configura explícitamente (`AGYSHIM_UPSTREAM`) y, si no hay
//! ninguno, el shim responde con la ruta canónica de MegaDaemon (el
//! gateway).  Nunca se escriben credenciales en el log: los cuerpos se
//! redactan por defecto (`AGYSHIM_REDACT=1`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Qué hacer con una petición que cumple una regla.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "accion", rename_all = "lowercase")]
pub enum Accion {
    /// Dejar pasar tal cual.
    Pasar,
    /// Devolver este cuerpo con este status.
    Sustituir { status: u16, cuerpo: String },
    /// Poner un valor en una ruta JSON del cuerpo (`a.b.c`).
    InyectarJson { ruta: String, valor: Value },
    /// Reescribir un texto por otro dentro del cuerpo.
    SustituirTexto { de: String, a: String },
    /// Cortar con un error JSON propio.
    Bloquear { status: u16, mensaje: String },
}

/// Condición que dispara una regla.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Condicion {
    /// La ruta contiene este texto.
    RutaContiene { texto: String },
    /// El modelo pedido es este.
    Modelo { nombre: String },
    /// El cuerpo contiene este texto.
    CuerpoContiene { texto: String },
    /// Siempre.
    Siempre,
}

/// Una regla del shim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regla {
    pub id: String,
    pub cuando: Condicion,
    pub hacer: Accion,
    #[serde(default = "por_defecto_true")]
    pub activa: bool,
    #[serde(default)]
    pub veces: u64,
}

/// Una petición interceptada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entrada {
    pub ts: u64,
    pub metodo: String,
    pub ruta: String,
    pub modelo: Option<String>,
    pub bytes_cuerpo: usize,
    pub status: u16,
    pub regla: Option<String>,
    /// Cuerpo (redactado si `AGYSHIM_REDACT=1`, que es el defecto).
    #[serde(default)]
    pub cuerpo: Option<String>,
    /// Body de la respuesta (también redactado).
    #[serde(default)]
    pub respuesta: Option<String>,
    pub ms: u64,
    /// El modelo que pidió el cliente (`megadaemon`, `gemini-2.5-pro`...).
    ///
    /// MEDIDO (2026-11-23): sin esto el NDJSON no distinguía "lo que la
    /// GUI cree que usa" de "lo que de verdad contestó".
    #[serde(default)]
    pub requested_model: Option<String>,
    /// El modelo **real** que contestó (id del proveedor).
    #[serde(default)]
    pub real_model: Option<String>,
    /// ¿El modelo real es DISTINTO del pedido?
    #[serde(default)]
    pub substituted: bool,
    /// ¿La sirvió el shim como proxy real (no sólo interceptada)?
    #[serde(default)]
    pub proxy: bool,
}

/// Estadísticas en memoria.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Estadisticas {
    pub peticiones: u64,
    pub interceptadas: u64,
    pub mentidas: u64,
    pub bloqueadas: u64,
    pub bytes: u64,
    /// Modelos pedidos y veces (lo que la GUI cree que usa).
    pub modelos: std::collections::BTreeMap<String, u64>,
    /// Peticiones servidas como **proxy real** (reenviadas al gateway).
    #[serde(default)]
    pub proxys: u64,
    /// Peticiones en las que el modelo real NO era el pedido.
    #[serde(default)]
    pub sustituciones: u64,
    /// Modelos **reales** que contestaron y veces.
    #[serde(default)]
    pub modelos_reales: std::collections::BTreeMap<String, u64>,
}

fn por_defecto_true() -> bool {
    true
}

fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn reglas_mapa() -> &'static Mutex<Vec<Regla>> {
    static R: OnceLock<Mutex<Vec<Regla>>> = OnceLock::new();
    R.get_or_init(|| {
        let mut v = Vec::new();
        let _ = restaurar(&mut v);
        Mutex::new(v)
    })
}

fn estadisticas_mapa() -> &'static Mutex<Estadisticas> {
    static E: OnceLock<Mutex<Estadisticas>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(Estadisticas::default()))
}

fn ruta_reglas() -> PathBuf {
    PathBuf::from(crate::cockpit_server::state_dir()).join("agyshim_rules.json")
}

fn ruta_log() -> PathBuf {
    PathBuf::from(crate::cockpit_server::state_dir()).join("agyshim.ndjson")
}

/// Carga las reglas desde disco.
pub fn restaurar(v: &mut Vec<Regla>) -> Result<(), String> {
    let txt = std::fs::read_to_string(ruta_reglas()).map_err(|e| e.to_string())?;
    let arr: Vec<Regla> = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
    *v = arr;
    Ok(())
}

/// Guarda las reglas.
///
/// MEDIDO (2026-11-23, auditoría de calidad): escribía con `fs::write` (que
/// TRUNCA el destino) y **ignoraba el error** (`let _ =`) ⇒ un corte a
/// mitad dejaba el fichero de reglas corrupto (y `restaurar` fallaba ⇒ el
/// shim dejaba de interceptar) sin que nadie se enterara.  Ahora la
/// escritura es atómica y el error se avisa por stderr.
pub fn guardar() {
    let m = reglas_mapa().lock().unwrap_or_else(|p| p.into_inner());
    let txt = serde_json::to_string_pretty(&*m).unwrap_or_default();
    drop(m);
    let _ = std::fs::create_dir_all(crate::cockpit_server::state_dir());
    let ruta = ruta_reglas();
    if let Err(e) = crate::util_fs::write_atomic(&ruta, txt.as_bytes(), 0o600) {
        eprintln!("[agyshim] no se pudieron guardar las reglas en {ruta:?}: {e}");
    }
}

/// Reglas actuales.
pub fn reglas() -> Vec<Regla> {
    reglas_mapa()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// Añade o reemplaza una regla (por id).
pub fn poner_regla(r: Regla) -> Result<(), String> {
    let mut m = reglas_mapa().lock().unwrap_or_else(|p| p.into_inner());
    match m.iter_mut().find(|x| x.id == r.id) {
        Some(existente) => *existente = r,
        None => m.push(r),
    }
    drop(m);
    guardar();
    Ok(())
}

/// Activa/desactiva una regla.
pub fn activar_regla(id: &str, activa: bool) -> Result<bool, String> {
    let mut m = reglas_mapa().lock().unwrap_or_else(|p| p.into_inner());
    match m.iter_mut().find(|x| x.id == id) {
        Some(r) => {
            r.activa = activa;
            drop(m);
            guardar();
            Ok(true)
        }
        None => {
            drop(m);
            Ok(false)
        }
    }
}

/// Borra una regla.
pub fn borrar_regla(id: &str) -> bool {
    let mut m = reglas_mapa().lock().unwrap_or_else(|p| p.into_inner());
    let antes = m.len();
    m.retain(|x| x.id != id);
    let b = m.len() != antes;
    drop(m);
    if b {
        guardar();
    }
    b
}

/// ¿Se redactan los cuerpos? (por defecto sí).
pub fn redacta() -> bool {
    std::env::var("AGYSHIM_REDACT")
        .ok()
        .map(|v| v != "0")
        .unwrap_or(true)
}

/// Redacta lo que parece una credencial.
///
/// MEDIDO (2026-11-23): la implementación se movió al crate compartido
/// `megadaemon-redact` (`crates/redact/`) porque el **payload de hooks** que
/// se inyecta dentro de la app (Antigravity) necesita EXACTAMENTE la misma
/// política de redacción: si el payload filtrara un token que este shim sí
/// tapa, el NDJSON del móvil acabaría en el bus.  Aquí sólo se **delega**:
/// mismo comportamiento, mismos casos de prueba (los del crate compartido).
///
/// El bug que motivó el crate (MEDIDO 2026-11-23, test rojo
/// `redacta_credenciales`): la credencial SE FILTRABA al log porque se
/// redactaba el NOMBRE de la clave pero no su VALOR.
pub fn redactar(texto: &str) -> String {
    // UNION (2026-11-23): el crate COMPARTIDO (`crates/redact`, que ya es la
    // fuente unica para el shim, el payload de hooks y el del inyector) MAS
    // la segunda pasada por FORMA de token que aporta la rama del MITM.
    crate::redact_forma::redactar_por_forma(&megadaemon_redact::redactar(texto))
}

/// Extrae el modelo pedido del cuerpo (JSON o texto).
pub fn modelo_de(cuerpo: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<Value>(cuerpo) {
        for ruta in ["model", "modelId", "chat_model_name", "modelVersion"] {
            if let Some(m) = v.get(ruta).and_then(|x| x.as_str()) {
                // 'model: "models/gemini-2.5-pro"' => 'gemini-2.5-pro'.
                // MEDIDO (2026-11-23): se devolvia el prefijo entero y el
                // shim pedia al router un modelo que no existe.
                return Some(m.rsplit('/').next().unwrap_or(m).to_string());
            }
        }
    }
    // busqueda simple en texto plano
    if let Some(i) = cuerpo.find("\"model\"") {
        let r = &cuerpo[i..];
        if let Some(j) = r.find(':') {
            let r2 = r[j + 1..].trim_start();
            if let Some(k) = r2.find('"') {
                let r3 = &r2[k + 1..];
                if let Some(f) = r3.find('"') {
                    return Some(r3[..f].to_string());
                }
            }
        }
    }
    None
}

/// ¿Se cumple la condición?
pub fn cumple(c: &Condicion, ruta: &str, modelo: Option<&str>, cuerpo: &str) -> bool {
    match c {
        Condicion::Siempre => true,
        Condicion::RutaContiene { texto } => ruta.contains(texto.as_str()),
        Condicion::Modelo { nombre } => modelo.map(|m| m == nombre).unwrap_or(false),
        Condicion::CuerpoContiene { texto } => cuerpo.contains(texto.as_str()),
    }
}

/// Aplica una acción a la respuesta.
///
/// `cuerpo` = cuerpo de la petición (para las condiciones), `respuesta` =
/// cuerpo de la respuesta real.
pub fn aplicar(accion: &Accion, ruta: &str, cuerpo: &str, respuesta: &str) -> (u16, String) {
    match accion {
        Accion::Pasar => (200, respuesta.to_string()),
        Accion::Sustituir { status, cuerpo: c } => (*status, c.clone()),
        Accion::Bloquear { status, mensaje } => (
            *status,
            json!({ "error": { "code": *status, "message": mensaje, "status": "BLOCKED_BY_MEGADAEMON" } })
                .to_string(),
        ),
        Accion::InyectarJson { ruta: r, valor } => {
            let mut v: Value = serde_json::from_str(respuesta).unwrap_or_else(|_| json!({}));
            set_json(&mut v, r, valor.clone());
            (200, v.to_string())
        }
        Accion::SustituirTexto { de, a } => {
            let mut s = respuesta.replace(de.as_str(), a.as_str());
            // si la respuesta era JSON, se re-serializa para que siga siendo valido
            if let Ok(v) = serde_json::from_str::<Value>(&s) {
                s = v.to_string();
            }
            let _ = (ruta, cuerpo);
            (200, s)
        }
    }
}

/// Pone un valor en una ruta `a.b.c` de un JSON.
pub fn set_json(v: &mut Value, ruta: &str, valor: Value) {
    let partes: Vec<&str> = ruta.split('.').filter(|s| !s.is_empty()).collect();
    if partes.is_empty() {
        *v = valor;
        return;
    }
    let mut cur = v;
    for p in &partes[..partes.len() - 1] {
        if !cur.is_object() {
            *cur = json!({});
        }
        cur = cur
            .as_object_mut()
            .unwrap()
            .entry(p.to_string())
            .or_insert_with(|| json!({}));
    }
    if !cur.is_object() {
        *cur = json!({});
    }
    cur.as_object_mut()
        .unwrap()
        .insert(partes[partes.len() - 1].to_string(), valor);
}

/// Registra una entrada (NDJSON) y actualiza estadísticas.
pub fn registrar(e: &Entrada) {
    {
        let mut s = estadisticas_mapa()
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        s.peticiones += 1;
        s.bytes += e.bytes_cuerpo as u64;
        if e.regla.is_some() {
            s.interceptadas += 1;
            if matches!(e.status, 0..=299) {
                s.mentidas += 1;
            } else if e.status >= 400 {
                s.bloqueadas += 1;
            }
        }
        if e.proxy {
            s.proxys += 1;
        }
        if e.substituted {
            s.sustituciones += 1;
        }
        if let Some(m) = &e.modelo {
            *s.modelos.entry(m.clone()).or_insert(0) += 1;
        }
        if let Some(m) = &e.real_model {
            *s.modelos_reales.entry(m.clone()).or_insert(0) += 1;
        }
    }
    let _ = std::fs::create_dir_all(crate::cockpit_server::state_dir());
    // El NDJSON puede acabar en una superficie compartida (el agente lo
    // lee): va 0600 y con las credenciales YA redactadas.
    let mut opciones = std::fs::OpenOptions::new();
    opciones.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opciones.mode(0o600);
    }
    if let Ok(mut f) = opciones.open(ruta_log()) {
        if let Ok(txt) = serde_json::to_string(e) {
            let _ = writeln!(f, "{txt}");
        }
    }
}

/// Estadísticas actuales.
pub fn estadisticas() -> Estadisticas {
    estadisticas_mapa()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// Últimas entradas del log (para el agente).
pub fn ultimas(n: usize) -> Vec<Value> {
    let Ok(txt) = std::fs::read_to_string(ruta_log()) else {
        return Vec::new();
    };
    let lineas: Vec<&str> = txt.lines().rev().take(n).collect();
    lineas
        .into_iter()
        .rev()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

/// **Núcleo del shim**: decide qué hacer con una petición.
///
/// Devuelve `(status, cuerpo, regla_aplicada)`.
pub fn decidir(
    metodo: &str,
    ruta: &str,
    cuerpo: &str,
    respuesta_real: &str,
) -> (u16, String, Option<String>) {
    let modelo = modelo_de(cuerpo);
    let reglas = reglas();
    for r in reglas.iter().filter(|x| x.activa) {
        if cumple(&r.cuando, ruta, modelo.as_deref(), cuerpo) {
            let (status, cuerpo_final) = aplicar(&r.hacer, ruta, cuerpo, respuesta_real);
            {
                let mut m = reglas_mapa().lock().unwrap_or_else(|p| p.into_inner());
                if let Some(rr) = m.iter_mut().find(|x| x.id == r.id) {
                    rr.veces += 1;
                }
            }
            let _ = metodo;
            return (status, cuerpo_final, Some(r.id.clone()));
        }
    }
    (200, respuesta_real.to_string(), None)
}

/// Upstream configurado (a dónde se reenvía).
pub fn upstream() -> String {
    std::env::var("AGYSHIM_UPSTREAM").unwrap_or_default()
}

// =========================================================================
// PROXY REAL (2026-11-23)
// =========================================================================
//
// HASTA AHORA el shim **interceptaba y registraba**: leía la petición,
// la anotaba en el NDJSON y devolvía... lo que ya venía.  No reenviaba
// nada: era un espejo, no un proxy.
//
// AHORA el shim **sirve de proxy de verdad**:
//
//   Antigravity (language server)
//        │  POST /v1internal:generateContent   (dialecto Gemini)
//        ▼
//   shim (agyshim::proxy)
//        │  registra (NDJSON, redactado) · aplica las reglas
//        ▼
//   GATEWAY CANÓNICO (llm_gateway)
//        │  decide proveedor · fallback real · credenciales del vault
//        ▼
//   proveedor real (Gemini / Claude / OpenAI-compatible)
//
// El shim NUNCA ve la credencial del proveedor: la tiene el vault del
// daemon (0600) y la usa `providers::execute_chat`.  El NDJSON lleva el
// modelo PEDIDO y el modelo REAL, redactados.

/// El modelo canónico del gateway (el alias que significa "routing completo").
pub const MODELO_CANONICO: &str = "megadaemon";

/// Backend REAL de Antigravity (MEDIDO en `liblanguage_server.so`: el
/// `baseUrl` por defecto es `https://daily-cloudcode-pa.googleapis.com`).
///
/// MEDIDO (2026-11-24): el language server pide, además de las rutas de
/// MODELO, rutas de **cuenta/sesión** (`/v1internal/buildWithGooglePlugins`,
/// `/v1internal/loadCodeAssist`, `/v1internal/userInfo`…).  Esas NO son
/// del modelo: son la cuenta de Google del usuario.  Si el shim las
/// rechaza con 404, la app no puede ni cargar su cascada ⇒ el modelo de
/// MegaDaemon nunca llega a usarse.
///
/// Para ésas se hace **passthrough**: se reenvía la petición al backend
/// real TAL CUAL (con el `Authorization` que ya envía la app) y se devuelve
/// la respuesta verbatim.  Se intercepta el MODELO, no la cuenta.
pub const BACKEND_REAL: &str = "https://daily-cloudcode-pa.googleapis.com";

/// ¿Esta ruta se pasa tal cual al backend real?
pub fn es_ruta_passthrough(ruta: &str) -> bool {
    let r = normalizar_ruta_proxy(ruta);
    r.starts_with("/v1internal") || r.starts_with("/v1beta")
}

/// Prefijos de ruta que el shim sirve como **proxy real**.
///
/// MEDIDO (2026-11-22, `liblanguage_server.so`): el language server usa
/// `/v1internal:generateContent` y `/v1internal:streamGenerateContent`
/// (dialecto Gemini, `genai` SDK de Google).  `/proxy` es el prefijo
/// genérico para los clientes que no hablan Gemini.
pub const PREFIJOS_PROXY: &[&str] = &["/v1internal", "/v1beta", "/proxy"];

/// ¿Esta ruta la sirve el shim como proxy real?
pub fn es_ruta_proxy(path: &str) -> bool {
    let p = path.trim();
    PREFIJOS_PROXY.iter().any(|pref| {
        let pref = *pref;
        p == pref || p.starts_with(&format!("{pref}/")) || p.starts_with(&format!("{pref}:"))
    })
}

/// Quita el prefijo `/proxy`: `/proxy/v1internal:generateContent` es lo
/// mismo que `/v1internal:generateContent`.
pub fn normalizar_ruta_proxy(path: &str) -> String {
    let p = path.trim();
    if let Some(r) = p.strip_prefix("/proxy") {
        if r.is_empty() {
            return "/".to_string();
        }
        if r.starts_with('/') || r.starts_with(':') {
            return r.to_string();
        }
    }
    p.to_string()
}

/// El dialecto que habla una ruta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialecto {
    /// Gemini (`genai`): `/v1internal:generateContent`, `/v1beta/models/x:…`.
    Gemini,
    /// OpenAI-compatible: `/proxy/v1/chat/completions`.
    OpenAi,
    /// Catálogo de modelos.
    Catalogo,
}

/// ¿Qué dialecto habla esta ruta?  `None` = el shim no la sirve.
pub fn dialecto_de(ruta: &str) -> Option<Dialecto> {
    let r = normalizar_ruta_proxy(ruta);
    // MEDIDO 2026-11-24 (movil): el LS pide el catalogo como
    // `/v1internal/listModels` (con barra) y antes nos devolvia 404.
    if r.ends_with(":fetchAvailableModels")
        || r.ends_with("/models")
        || r == "/models"
        || r.ends_with("/listModels")
        || r.ends_with(":listModels")
    {
        return Some(Dialecto::Catalogo);
    }
    if r.contains("/chat/completions") {
        return Some(Dialecto::OpenAi);
    }
    if crate::llm_gateway::gemini_route(&r).is_some() {
        return Some(Dialecto::Gemini);
    }
    None
}

/// Una petición que entra al shim.
#[derive(Debug, Clone, Default)]
pub struct Peticion {
    pub metodo: String,
    pub ruta: String,
    pub cabeceras: Vec<(String, String)>,
    pub cuerpo: String,
}

/// La respuesta que el shim devuelve (ya en el dialecto del cliente).
#[derive(Debug, Clone)]
pub struct Respuesta {
    pub status: u16,
    pub content_type: String,
    pub cuerpo: String,
    /// El modelo que pidió el cliente.
    pub requested_model: String,
    /// El modelo REAL que contestó (vacío si no se pudo saber).
    pub real_model: String,
    /// ¿El modelo real es distinto del pedido?
    pub substituted: bool,
    /// La regla que se aplicó (si alguna).
    pub regla: Option<String>,
}

/// Reenvía la petición al backend real de Antigravity y devuelve la
/// respuesta tal cual.
///
/// La autenticación la pone la APP (cabecera `Authorization` propia): el
/// daemon no guarda ni ve credenciales de Google, sólo las reenvía.
pub fn passthrough(req: &Peticion) -> Result<Respuesta, String> {
    let ruta = normalizar_ruta_proxy(&req.ruta);
    let url = format!("{}{}", BACKEND_REAL, ruta);
    let agente = agente_passthrough();
    let metodo = req.metodo.trim().to_ascii_uppercase();
    // Cabeceras que se reenvían (las de salto las pone ureq).
    let cabeceras: Vec<(&str, &str)> = req
        .cabeceras
        .iter()
        .filter(|(k, _)| {
            !matches!(
                k.to_ascii_lowercase().as_str(),
                "host" | "content-length" | "connection" | "transfer-encoding" | "accept-encoding"
            )
        })
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    // MEDIDO (2026-11-24, error E0308 del CI): en ureq 3 el builder tiene
    // TIPADO POR ESTADO (`WithBody` / `WithoutBody`) ⇒ los métodos con cuerpo
    // y sin cuerpo NO se pueden mezclar en un mismo `match`.  Cada rama va
    // por su cuenta.
    macro_rules! con_cabeceras {
        ($req:expr) => {{
            let mut p = $req;
            for (k, v) in &cabeceras {
                p = p.header(*k, *v);
            }
            p
        }};
    }
    let resp = match metodo.as_str() {
        "GET" => con_cabeceras!(agente.get(&url)).call(),
        "DELETE" => con_cabeceras!(agente.delete(&url)).call(),
        "PUT" => con_cabeceras!(agente.put(&url)).send(req.cuerpo.as_bytes()),
        "PATCH" => con_cabeceras!(agente.patch(&url)).send(req.cuerpo.as_bytes()),
        // MEDIDO (2026-11-24, error E0599): en ureq 3 el builder de POST es
        // `WithBody` y NO tiene `.call()`: siempre se envía (el cuerpo vacío
        // es un POST sin cuerpo ✔).
        _ => con_cabeceras!(agente.post(&url)).send(req.cuerpo.as_bytes()),
    }
    .map_err(|e| format!("passthrough a {}: {}", BACKEND_REAL, e))?;
    let status = resp.status().as_u16();
    // MEDIDO (2026-11-24, error E0599): en ureq 3 no hay `resp.content_type()`;
    // la cabecera se lee de `headers()`.
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let cuerpo = resp
        .into_body()
        .read_to_string()
        .map_err(|e| format!("passthrough: lectura del cuerpo: {e}"))?;
    Ok(Respuesta {
        status,
        content_type,
        cuerpo,
        requested_model: MODELO_CANONICO.to_string(),
        real_model: String::new(),
        substituted: false,
        regla: Some("passthrough".to_string()),
    })
}

/// Agente HTTP para el passthrough (TLS verificado: es Google de verdad).
fn agente_passthrough() -> ureq::Agent {
    use std::time::Duration;
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::Rustls)
        .build();
    let cfg = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(90)))
        .timeout_connect(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .build();
    ureq::Agent::new_with_config(cfg)
}

/// **Proxy real**: atiende la ruta y la reenvía al proveedor real.
///
/// La reenvía **pasando por el gateway canónico** (`llm_gateway`), que es
/// quien decide el proveedor, hace el *fallback* real y usa las
/// credenciales del vault (el shim nunca las ve).
///
/// Devuelve la respuesta **en el dialecto que pidió el cliente** (Gemini
/// ⇒ `candidates`, OpenAI ⇒ `choices`) con el bloque de trazabilidad
/// `megadaemon` (`requested_model`, `real_model`, `substituted`).
pub fn proxy(req: &Peticion) -> Result<Respuesta, String> {
    let ruta = normalizar_ruta_proxy(&req.ruta);
    let dialecto = match dialecto_de(&req.ruta) {
        Some(d) => d,
        None => {
            // Ruta de cuenta/sesión (no de modelo): passthrough al backend
            // real con las credenciales que ya envía la app.
            if es_ruta_passthrough(&req.ruta) {
                return passthrough(req);
            }
            return Err(format!("el shim no sirve la ruta {}", req.ruta));
        }
    };
    let t0 = std::time::Instant::now();
    let cuerpo: Value = serde_json::from_str(&req.cuerpo).unwrap_or(Value::Null);

    let mut requested = MODELO_CANONICO.to_string();
    let mut real = String::new();
    let mut sustituido = false;
    let mut content_type = "application/json".to_string();
    let mut salida;

    match dialecto {
        Dialecto::Catalogo => {
            let v = crate::antigravity_endpoint::catalogo_gemini();
            salida = v.to_string();
        }
        Dialecto::OpenAi => {
            let mut v = crate::llm_gateway::chat_completions_con_traza(&cuerpo)
                .map_err(|e| format!("gateway OpenAI: {}", e.message))?;
            requested = cuerpo
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or(MODELO_CANONICO)
                .trim()
                .to_string();
            if requested.is_empty() {
                requested = MODELO_CANONICO.to_string();
            }
            real = v
                .pointer("/megadaemon/provider_id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            sustituido = v
                .pointer("/megadaemon/substituted")
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            salida = v.to_string();
        }
        Dialecto::Gemini => {
            let r = crate::llm_gateway::gemini_route(&ruta);
            let stream = r.as_ref().map(|x| x.stream).unwrap_or(false);
            let modelo_ruta = r.as_ref().map(|x| x.model.clone()).unwrap_or_default();
            requested = if modelo_ruta.trim().is_empty() {
                MODELO_CANONICO.to_string()
            } else {
                modelo_ruta.trim().to_string()
            };
            // Si el cliente pide un modelo CONCRETO, ese va primero en la
            // cascada; si pide el canónico, decide el router.
            let solicitado: Option<&str> = if crate::llm_gateway::es_alias_megadaemon(&requested) {
                None
            } else {
                Some(requested.as_str())
            };
            if stream {
                let mut v = crate::llm_gateway::gemini_generate_content_con_modelo_for(
                    &ruta, &cuerpo, solicitado,
                )
                .map_err(|e| format!("gateway Gemini: {}", e.message))?;
                real = v
                    .pointer("/megadaemon/provider_id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                sustituido = v
                    .pointer("/megadaemon/substituted")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);
                let sse = crate::llm_gateway::gemini_a_sse(&v);
                content_type = "text/event-stream; charset=utf-8".to_string();
                salida = sse;
            } else {
                let mut v = crate::llm_gateway::gemini_generate_content_con_modelo_for(
                    &ruta, &cuerpo, solicitado,
                )
                .map_err(|e| format!("gateway Gemini: {}", e.message))?;
                real = v
                    .pointer("/megadaemon/provider_id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                sustituido = v
                    .pointer("/megadaemon/substituted")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);
                salida = v.to_string();
            }
        }
    }

    // Las reglas del shim se aplican ENCIMA de la respuesta del gateway:
    // es el punto donde se puede mentir a la GUI.
    let (status, cuerpo_final, regla) = decidir(&req.metodo, &req.ruta, &req.cuerpo, &salida);

    let redacta = redacta();
    registrar(&Entrada {
        ts: ahora(),
        metodo: req.metodo.clone(),
        ruta: req.ruta.clone(),
        modelo: Some(requested.clone()),
        bytes_cuerpo: req.cuerpo.len(),
        status,
        regla: regla.clone(),
        cuerpo: if redacta {
            Some(redactar(&req.cuerpo))
        } else {
            None
        },
        respuesta: if redacta {
            Some(redactar(&cuerpo_final))
        } else {
            None
        },
        ms: t0.elapsed().as_millis() as u64,
        requested_model: Some(requested.clone()),
        real_model: if real.is_empty() { None } else { Some(real.clone()) },
        substituted: sustituido,
        proxy: true,
    });

    Ok(Respuesta {
        status,
        content_type,
        cuerpo: cuerpo_final,
        requested_model: requested,
        real_model: real,
        substituted: sustituido,
        regla,
    })
}

/// Estado del proxy para el agente/MCP: qué sabe y qué ha visto.
pub fn estado_proxy() -> Value {
    let s = estadisticas();
    json!({
        "ok": true,
        "prefijos_proxy": PREFIJOS_PROXY,
        "modelo_canonico": MODELO_CANONICO,
        "upstream_configurado": !upstream().is_empty(),
        "redacta": redacta(),
        "peticiones": s.peticiones,
        "proxys": s.proxys,
        "sustituciones": s.sustituciones,
        "modelos_pedidos": s.modelos,
        "modelos_reales": s.modelos_reales,
        "reglas": reglas().len(),
        "nota": "el shim reenvia por el gateway canonico: el proveedor lo elige llm_gateway y la credencial vive en el vault (0600), nunca en el NDJSON.",
    })
}

/// Procesa una petición del LS **de punta a punta**:
/// registra, aplica las reglas y devuelve la respuesta que verá Antigravity.
///
/// Si no hay upstream configurado, devuelve la respuesta real sin tocar.
pub fn procesar(metodo: &str, ruta: &str, cuerpo: &str) -> (u16, String) {
    let t0 = std::time::Instant::now();
    let up = upstream();
    let real = if up.is_empty() {
        // Sin upstream: el shim no reenvia (no inventa datos).
        cuerpo.to_string()
    } else {
        let url = format!("{}{}", up.trim_end_matches('/'), ruta);
        let mut o = crate::http_transfer::Opciones::default();
        o.timeout_s = 300;
        // MEDIDO (2026-11-23): era un GET ⇒ un proxy que reenvía un POST
        // con un GET no funciona.  Ahora es POST de verdad y el status
        // del upstream se conserva (un 429 ya no se pierde).
        match crate::http_transfer::post_texto(&url, cuerpo, &o) {
            Ok((st, t)) => {
                if st >= 400 {
                    format!("{{\"error\":{{\"code\":{st},\"message\":\"upstream {st}\"}}}}")
                } else {
                    t
                }
            }
            Err(e) => format!("{{\"error\":{{\"code\":502,\"message\":\"{e}\"}}}}"),
        }
    };
    let (status, final_cuerpo, regla) = decidir(metodo, ruta, cuerpo, &real);
    let redacta = redacta();
    registrar(&Entrada {
        ts: ahora(),
        metodo: metodo.to_string(),
        // Campos nuevos (2026-11-23, rama del MITM): el modelo pedido, el
        // REAL que contesto, si se sustituyo y si el shim sirvio de proxy.
        requested_model: None,
        real_model: None,
        substituted: false,
        proxy: false,
        // MEDIDO (2026-11-23, auditoría de calidad): la RUTA se guardaba en
        // claro mientras que el cuerpo y la respuesta pasaban por el
        // redactor.  La ruta de una llamada a la API puede llevar la
        // credencial (los proveedores la ponen en la query: `?key=...`,
        // `?api_key=...`) ⇒ el NDJSON del shim acababa con el secreto.
        // Ahora la ruta pasa por el MISMO redactor que el resto.
        //
        // ⚠️ NO VERIFICADO: el redactor actual sólo conoce los formatos
        // "nombre: valor" (JSON) y "nombre valor"; NO cubre el formato
        // query-string (`?key=valor`), que se queda como está.  Se deja
        // anotado en docs/AUDITORIA-CALIDAD-2026-11-23.md.
        ruta: if redacta {
            redactar(ruta)
        } else {
            ruta.to_string()
        },
        modelo: modelo_de(cuerpo),
        bytes_cuerpo: cuerpo.len(),
        status,
        regla,
        cuerpo: if redacta {
            Some(redactar(cuerpo))
        } else {
            None
        },
        respuesta: if redacta {
            Some(redactar(&final_cuerpo))
        } else {
            None
        },
        ms: t0.elapsed().as_millis() as u64,
    });
    (status, final_cuerpo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r_tmp(id: &str) -> Regla {
        Regla {
            id: id.to_string(),
            cuando: Condicion::Siempre,
            hacer: Accion::Pasar,
            activa: false,
            veces: 0,
        }
    }

    #[test]
    fn el_modelo_se_extrae_del_cuerpo() {
        assert_eq!(
            modelo_de(r#"{"model":"models/gemini-2.5-pro"}"#).as_deref(),
            Some("gemini-2.5-pro")
        );
        assert_eq!(
            modelo_de(r#"{"model":"minimax-m3.1-flash"}"#).as_deref(),
            Some("minimax-m3.1-flash")
        );
        assert_eq!(modelo_de(r#"{"otra":"cosa"}"#), None);
    }

    #[test]
    fn redacta_credenciales() {
        let t = redactar(r#"{"api_key":"sk-cp-MUY-SECRETO"}"#);
        assert!(!t.contains("MUY-SECRETO"), "no debe filtrar la clave: {t}");
        assert!(t.contains("REDACTADO"));
    }

    #[test]
    fn la_condicion_se_evalua_bien() {
        assert!(cumple(
            &Condicion::RutaContiene {
                texto: "v1internal".into()
            },
            "/v1internal/generateContent",
            None,
            ""
        ));
        assert!(cumple(
            &Condicion::Modelo {
                nombre: "megadaemon".into()
            },
            "/x",
            Some("megadaemon"),
            ""
        ));
        assert!(!cumple(
            &Condicion::CuerpoContiene {
                texto: "zzz".into()
            },
            "/x",
            None,
            "abc"
        ));
    }

    #[test]
    fn inyectar_json_crea_el_ruta() {
        let mut v = json!({ "a": { "b": 1 } });
        set_json(&mut v, "a.c", json!(42));
        assert_eq!(v["a"]["c"], 42);
        assert_eq!(v["a"]["b"], 1);
    }

    #[test]
    fn sustituir_texto_mantiene_el_json_valido() {
        let (st, c) = aplicar(
            &Accion::SustituirTexto {
                de: "megadaemon".into(),
                a: "gemini-3".into(),
            },
            "/x",
            "",
            r#"{"model":"megadaemon"}"#,
        );
        assert_eq!(st, 200);
        let v: Value = serde_json::from_str(&c).expect("JSON valido");
        assert_eq!(v["model"], "gemini-3");
    }

    #[test]
    fn bloquear_devuelve_error_propio() {
        let (st, c) = aplicar(
            &Accion::Bloquear {
                status: 403,
                mensaje: "no".into(),
            },
            "/x",
            "",
            "{}",
        );
        assert_eq!(st, 403);
        assert!(c.contains("BLOCKED_BY_MEGADAEMON"));
    }

    #[test]
    fn una_regla_activa_cambia_la_respuesta() {
        // se usa el mapa real: se limpia al final
        let id = format!("test-{}", ahora());
        poner_regla(Regla {
            id: id.clone(),
            cuando: Condicion::RutaContiene {
                texto: "v1internal".into(),
            },
            hacer: Accion::Sustituir {
                status: 200,
                cuerpo: r#"{"model":"megadaemon"}"#.into(),
            },
            activa: true,
            veces: 0,
        })
        .unwrap();
        let (st, c, r) = decidir(
            "POST",
            "/v1internal/generateContent",
            "{}",
            r#"{"model":"real"}"#,
        );
        assert_eq!(st, 200);
        assert!(c.contains("megadaemon"));
        assert_eq!(r.as_deref(), Some(id.as_str()));
        borrar_regla(&id);
    }

    // -----------------------------------------------------------------
    // PROXY REAL (2026-11-23)
    // -----------------------------------------------------------------

    #[test]
    fn el_shim_sirve_las_rutas_del_language_server() {
        // MEDIDO (2026-11-22, `liblanguage_server.so`).
        assert!(es_ruta_proxy("/v1internal:generateContent"));
        assert!(es_ruta_proxy("/v1internal:streamGenerateContent"));
        assert!(es_ruta_proxy("/v1internal/fetchAvailableModels"));
        assert!(es_ruta_proxy("/proxy/v1/chat/completions"));
        assert!(es_ruta_proxy("/v1beta/models/gemini-2.5-pro:generateContent"));
        // Y NO las que no son suyas.
        assert!(!es_ruta_proxy("/v1/chat/completions"));
        assert!(!es_ruta_proxy("/health"));
        assert!(!es_ruta_proxy("/api/antigravity/sessions"));
        assert!(!es_ruta_proxy("/proxyxyz"));
    }

    #[test]
    fn el_prefijo_proxy_se_normaliza() {
        assert_eq!(
            normalizar_ruta_proxy("/proxy/v1internal:generateContent"),
            "/v1internal:generateContent"
        );
        assert_eq!(
            normalizar_ruta_proxy("/v1internal:generateContent"),
            "/v1internal:generateContent"
        );
    }

    #[test]
    fn el_dialecto_se_reconoce() {
        assert_eq!(
            dialecto_de("/v1internal:generateContent"),
            Some(Dialecto::Gemini)
        );
        assert_eq!(
            dialecto_de("/v1beta/models/x:streamGenerateContent"),
            Some(Dialecto::Gemini)
        );
        assert_eq!(
            dialecto_de("/proxy/v1/chat/completions"),
            Some(Dialecto::OpenAi)
        );
        assert_eq!(
            dialecto_de("/v1internal:fetchAvailableModels"),
            Some(Dialecto::Catalogo)
        );
        assert_eq!(dialecto_de("/otra/cosa"), None);
    }

    #[test]
    fn las_rutas_de_cuenta_se_pasan_al_backend_real() {
        // MEDIDO (2026-11-24): el LS pide `/v1internal/buildWithGooglePlugins`
        // (la cascada del agente) y `/v1internal/` (loadCodeAssist): son
        // rutas de CUENTA, no de modelo ⇒ pasan tal cual a Google.
        assert!(dialecto_de("/v1internal/buildWithGooglePlugins").is_none());
        assert!(es_ruta_passthrough("/v1internal/buildWithGooglePlugins"));
        assert!(es_ruta_passthrough("/v1internal/loadCodeAssist"));
        // Las de MODELO las intercepta el gateway (nunca pasan a Google).
        assert!(dialecto_de("/v1internal:generateContent").is_some());
        assert!(dialecto_de("/v1internal:streamGenerateContent").is_some());
        assert!(dialecto_de("/v1internal:fetchAvailableModels").is_some());
    }

    #[test]
    fn la_redaccion_cubre_las_credenciales_por_forma() {
        // MEDIDO (2026-11-23): la primera pasada solo conocia los NOMBRES
        // de los campos; una credencial suelta se colaba en el NDJSON.
        let t = redactar(r#"{"nota":"key sk-cp-ABCDEFGHIJKLMNOP y token AIzaSyABCDEFGHIJKLMNOP"}"#);
        assert!(!t.contains("ABCDEFGHIJKLMNOP"), "no debe filtrar: {t}");
        assert!(t.contains("REDACTADO"));
        // Y no toca el texto normal.
        let t2 = redactar("esto es sk-3 corto y AIza algo");
        assert!(t2.contains("sk-3"), "no debe redaccionar texto normal: {t2}");
    }

    #[test]
    fn la_redaccion_por_forma_vive_en_un_modulo_propio() {
        // El modulo `redact_forma` es el que debe acabar en el crate
        // compartido `megadaemon-redact` (fuente unica de la politica).
        let t = crate::redact_forma::redactar_por_forma("token suelta sk-cp-ABCDEFGHIJKLMNOP");
        assert!(!t.contains("ABCDEFGHIJKLMNOP"));
    }

    #[test]
    fn la_traza_del_modelo_distingue_pedido_de_real() {
        // El alias canonico SIEMPRE es una sustitucion (el alias no es un
        // modelo: es el router).
        let mut v = json!({"megadaemon": {"provider_id": "minimax-m3.1-flash"}});
        crate::llm_gateway::anotar_traza(&mut v, "megadaemon");
        assert_eq!(v["megadaemon"]["requested_model"], "megadaemon");
        assert_eq!(v["megadaemon"]["real_model"], "minimax-m3.1-flash");
        assert_eq!(v["megadaemon"]["substituted"], true);

        // Si el modelo pedido es el que contesta, NO hay sustitucion.
        let mut v2 = json!({"megadaemon": {"provider_id": "minimax-m3.1-flash"}});
        crate::llm_gateway::anotar_traza(&mut v2, "minimax-m3.1-flash");
        assert_eq!(v2["megadaemon"]["substituted"], false);

        // Y si contesto otro, si la hay.
        let mut v3 = json!({"megadaemon": {"provider_id": "groq-free"}});
        crate::llm_gateway::anotar_traza(&mut v3, "minimax-m3.1-flash");
        assert_eq!(v3["megadaemon"]["substituted"], true);
        assert_eq!(v3["megadaemon"]["real_model"], "groq-free");
    }

    #[test]
    fn los_alias_del_gateway_se_reconocen() {
        assert!(crate::llm_gateway::es_alias_megadaemon("megadaemon"));
        assert!(crate::llm_gateway::es_alias_megadaemon("MEGADAEMON"));
        assert!(crate::llm_gateway::es_alias_megadaemon("auto"));
        assert!(!crate::llm_gateway::es_alias_megadaemon("minimax-m3.1-flash"));
        assert!(!crate::llm_gateway::es_alias_megadaemon(""));
    }

    #[test]
    fn las_reglas_se_persisten() {
        let id = format!("test-persistencia-{}", ahora());
        let mut r = r_tmp(&id);
        r.activa = true;
        poner_regla(r).unwrap();
        let encontradas = reglas();
        assert!(encontradas.iter().any(|x| x.id == id));
        borrar_regla(&id);
    }
}
