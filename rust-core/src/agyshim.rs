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
    megadaemon_redact::redactar(texto)
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
        if let Some(m) = &e.modelo {
            *s.modelos.entry(m.clone()).or_insert(0) += 1;
        }
    }
    let _ = std::fs::create_dir_all(crate::cockpit_server::state_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ruta_log())
    {
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
        match crate::http_transfer::get_texto(&url, &o) {
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
