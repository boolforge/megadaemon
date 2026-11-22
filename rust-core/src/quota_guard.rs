//! Guard de cuotas: saber cuánto queda, avisar ANTES de que se corte y
//! **parar de forma elegante** (nunca cortar el flujo a mitad).
//!
//! PROBLEMA REAL (2026-11-22): los agentes de la flota se quedaron **sin
//! cuota a mitad de un bloque de trabajo** y perdieron todo lo que no
//! estaba commiteado (varios cientos de líneas).  Este módulo evita que
//! vuelva a pasar:
//!
//! 1. **Observa**: registra el uso real (tokens) y parsea los errores de
//!    cuota que devuelve el proveedor (429, "quota exceeded", "resets at",
//!    "retry after", 402 sin créditos).
//! 2. **Avisa**: cuando el consumo pasa del umbral de aviso, el estado
//!    viaja **dentro de la respuesta del chat** (campo `quota`) ⇒ el
//!    agente lo sabe **sin gastar un solo token extra** en una llamada
//!    aparte.  Hay además un snapshot muy compacto para quien lo pida.
//! 3. **Ahorra**: en modo "ahorro" el daemon prioriza lo local (SEERE) y
//!    deja de gastar cuota en tareas nuevas.
//! 4. **Pausa con elegantía**: en modo "pausa" **no** se corta el trabajo
//!    en curso: se deja terminar el paso actual, la tarea se marca
//!    `resume_after = reset_at` y se reanuda sola cuando la cuota vuelve.
//!
//! Umbrales (configurables por entorno):
//!   `MD_QUOTA_AVISO=80` `MD_QUOTA_AHORRO=90` `MD_QUOTA_PAUSA=98`

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

/// Nivel de la cuota de un proveedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Nivel {
    /// Todo bien.
    Normal,
    /// Cerca del límite: avisar al agente.
    Aviso,
    /// Muy cerca: no gastar en tareas nuevas.
    Ahorro,
    /// Cortada: pausar con elegancia y esperar al reset.
    Pausa,
}

impl Nivel {
    pub fn as_str(self) -> &'static str {
        match self {
            Nivel::Normal => "normal",
            Nivel::Aviso => "aviso",
            Nivel::Ahorro => "ahorro",
            Nivel::Pausa => "pausa",
        }
    }
    pub fn from_str(s: &str) -> Nivel {
        match s {
            "aviso" => Nivel::Aviso,
            "ahorro" => Nivel::Ahorro,
            "pausa" => Nivel::Pausa,
            _ => Nivel::Normal,
        }
    }
}

/// Una ventana de cuota (5 h, semanal, diaria...).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ventana {
    /// Nombre de la ventana ("5h", "semanal").
    pub nombre: String,
    /// Porcentaje consumido (0..=100).
    pub pct_usado: f64,
    /// Epoch del reset (si el proveedor lo dice).
    #[serde(default)]
    pub reset_at: Option<u64>,
    /// De dónde sale el dato: "error429", "probe", "estimado", "config".
    #[serde(default = "fuente_por_defecto")]
    pub fuente: String,
}

fn fuente_por_defecto() -> String {
    "config".to_string()
}

/// Estado de la cuota de un proveedor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Estado {
    pub provider: String,
    pub nivel: Nivel,
    pub ventanas: Vec<Ventana>,
    /// Detalle legible (por qué se pause, cuándo vuelve...).
    #[serde(default)]
    pub detalle: Option<String>,
    #[serde(default)]
    pub actualizado_at: u64,
}

/// Error de cuota detectado en la respuesta del proveedor.
#[derive(Debug, Clone)]
pub struct ErrorCuota {
    /// Texto normalizado para el agente.
    pub motivo: String,
    /// Epoch del reset, si lo dice el proveedor.
    pub reset_at: Option<u64>,
    /// Segundos que dice esperar el proveedor.
    pub retry_after: Option<u64>,
}

fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn umbral(nombre: &str, defecto: f64) -> f64 {
    std::env::var(nombre)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(defecto)
}

/// Umbral de aviso (%).
pub fn umbral_aviso() -> f64 {
    umbral("MD_QUOTA_AVISO", 80.0)
}
/// Umbral de ahorro (%).
pub fn umbral_ahorro() -> f64 {
    umbral("MD_QUOTA_AHORRO", 90.0)
}
/// Umbral de pausa (%).
pub fn umbral_pausa() -> f64 {
    umbral("MD_QUOTA_PAUSA", 98.0)
}

/// Nivel correspondiente a un porcentaje consumido.
pub fn nivel_para(pct: f64) -> Nivel {
    if pct >= umbral_pausa() {
        Nivel::Pausa
    } else if pct >= umbral_ahorro() {
        Nivel::Ahorro
    } else if pct >= umbral_aviso() {
        Nivel::Aviso
    } else {
        Nivel::Normal
    }
}

type Mapa = BTreeMap<String, Estado>;

fn mapa() -> &'static Mutex<Mapa> {
    static M: OnceLock<Mutex<Mapa>> = OnceLock::new();
    M.get_or_init(|| {
        let mut m = Mapa::new();
        let _ = restaurar_en(&mut m);
        Mutex::new(m)
    })
}

fn ruta_estado() -> String {
    format!("{}/quota.json", crate::cockpit_server::state_dir())
}

fn restaurar_en(m: &mut Mapa) -> Result<(), String> {
    let txt = std::fs::read_to_string(ruta_estado()).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            if let Ok(e) = serde_json::from_value::<Estado>(val.clone()) {
                m.insert(k.clone(), e);
            }
        }
    }
    Ok(())
}

/// Guarda el estado en disco (para que sobreviva a un reinicio).
pub fn persistir() {
    let m = mapa().lock().unwrap_or_else(|p| p.into_inner());
    let v = serde_json::to_value(&*m).unwrap_or_else(|_| json!({}));
    drop(m);
    let _ = std::fs::create_dir_all(crate::cockpit_server::state_dir());
    let _ = std::fs::write(ruta_estado(), serde_json::to_string_pretty(&v).unwrap_or_default());
}

/// Extrae el código HTTP de un mensaje de error del proveedor.
///
/// El error de `call_openai_http` tiene la forma "HTTP 429: ...".
pub fn status_de_error(msg: &str) -> Option<u16> {
    let i = msg.find("HTTP ")? + 5;
    let resto = &msg[i..];
    let fin = resto.find(|c: char| !c.is_ascii_digit()).unwrap_or(resto.len());
    resto[..fin].trim().parse::<u16>().ok()
}

/// Detecta un error de cuota en el cuerpo de la respuesta de un proveedor.
///
/// Reconoce los mensajes reales que devuelven los proveedores:
/// * HTTP 429 con "rate limit" o "quota exceeded";
/// * "You have exceeded your quota" / "insufficient credits" / 402;
/// * "resets at 5pm" / "resets at 1795130605";
/// * "retry after 42".
pub fn parsear_error_cuota(cuerpo: &str) -> Option<ErrorCuota> {
    let t = cuerpo.to_ascii_lowercase();
    let es_cuota = t.contains("quota")
        || t.contains("rate limit")
        || t.contains("rate_limit")
        || t.contains("too many requests")
        || t.contains("insufficient credit")
        || t.contains("insufficient balance")
        || t.contains("exceeded your")
        || t.contains("billing")
        || t.contains("resource_exhausted");
    if !es_cuota {
        return None;
    }
    let mut reset_at = None;
    // "resets at <epoch>" o "reset at <epoch>"
    for marca in ["resets at ", "reset at ", "resets_in ", "resets in "] {
        if let Some(i) = t.find(marca) {
            let resto = &t[i + marca.len()..];
            let fin = resto
                .find(|c: char| !c.is_ascii_digit() && c != ':' && c != ' ' && c != '.')
                .unwrap_or(resto.len());
            let frag = resto[..fin].trim();
            if let Ok(epoch) = frag.parse::<u64>() {
                if epoch > 1_000_000_000 {
                    reset_at = Some(epoch);
                }
            } else if frag.contains(':') {
                // "5pm" / "17:30" => hoy o mañana a esa hora local.
                if let Some(h) = frag.split(':').next().and_then(|h| h.parse::<u64>().ok()) {
                    let hoy = ahora() / 86_400 * 86_400;
                    let cand = hoy + h * 3_600;
                    reset_at = Some(if cand > ahora() { cand } else { cand + 86_400 });
                }
            }
            break;
        }
    }
    let mut retry_after = None;
    for marca in ["retry after ", "retry-after: ", "retry_after=", "try again in "] {
        if let Some(i) = t.find(marca) {
            let resto = &t[i + marca.len()..];
            let fin = resto
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(resto.len());
            if let Ok(s) = resto[..fin].trim().parse::<u64>() {
                retry_after = Some(s);
            }
            break;
        }
    }
    let motivo = cuerpo
        .lines()
        .find(|l| {
            let l = l.to_ascii_lowercase();
            l.contains("quota") || l.contains("rate limit") || l.contains("credit")
        })
        .unwrap_or("cuota agotada")
        .trim()
        .chars()
        .take(160)
        .collect::<String>();
    Some(ErrorCuota {
        motivo,
        reset_at,
        retry_after,
    })
}

/// Registra un error de cuota y pone al proveedor en pausa hasta el reset.
///
/// `status` es el código HTTP (429, 402, 403...).
pub fn registrar_error_cuota(provider: &str, status: u16, cuerpo: &str) -> Option<ErrorCuota> {
    let e = parsear_error_cuota(cuerpo)?;
    let reset_at = e.reset_at.or_else(|| {
        e.retry_after.map(|s| ahora() + s)
    });
    let pct = if status == 429 { 100.0 } else { umbral_pausa() };
    let ventana = Ventana {
        nombre: "cupo".to_string(),
        pct_usado: pct,
        reset_at,
        fuente: format!("error{status}"),
    };
    let mut m = mapa().lock().unwrap_or_else(|p| p.into_inner());
    let e2 = m.entry(provider.to_string()).or_insert_with(|| Estado {
        provider: provider.to_string(),
        nivel: Nivel::Normal,
        ventanas: Vec::new(),
        detalle: None,
        actualizado_at: 0,
    });
    e2.ventanas.retain(|v| v.nombre != "cupo");
    e2.ventanas.push(ventana);
    e2.nivel = peor_nivel(&e2.ventanas);
    e2.detalle = Some(format!(
        "{} (HTTP {status}){}",
        e.motivo,
        match reset_at {
            Some(r) => format!("; vuelve {}", r),
            None => String::new(),
        }
    ));
    e2.actualizado_at = ahora();
    drop(m);
    persistir();
    Some(e)
}

fn peor_nivel(ventanas: &[Ventana]) -> Nivel {
    let mut n = Nivel::Normal;
    for v in ventanas {
        let l = nivel_para(v.pct_usado);
        if l > n {
            n = l;
        }
    }
    n
}

/// Registra el uso real (tokens) como porcentaje de un límite conocido.
///
/// Si el límite no se conoce, el dato se guarda igualmente como
/// "estimado" y no cambia el nivel (no se inventa un porcentaje).
pub fn registrar_uso(provider: &str, tokens: u64, limite: Option<u64>) {
    let mut m = mapa().lock().unwrap_or_else(|p| p.into_inner());
    let e = m.entry(provider.to_string()).or_insert_with(|| Estado {
        provider: provider.to_string(),
        nivel: Nivel::Normal,
        ventanas: Vec::new(),
        detalle: None,
        actualizado_at: 0,
    });
    match limite {
        Some(l) if l > 0 => {
            let pct = (tokens as f64 / l as f64 * 100.0).min(100.0);
            e.ventanas.retain(|v| v.nombre != "uso");
            e.ventanas.push(Ventana {
                nombre: "uso".to_string(),
                pct_usado: pct,
                reset_at: None,
                fuente: "estimado".to_string(),
            });
        }
        Some(_) => {}
        None => {
            e.ventanas.retain(|v| v.nombre != "tokens");
            e.ventanas.push(Ventana {
                nombre: "tokens".to_string(),
                pct_usado: 0.0,
                reset_at: None,
                fuente: "tokens".to_string(),
            });
        }
    }
    e.nivel = peor_nivel(&e.ventanas);
    e.actualizado_at = ahora();
}

/// Fija las ventanas de cuota de un proveedor (sonda explícita o config).
pub fn establecer_ventanas(provider: &str, ventanas: Vec<Ventana>) {
    let mut m = mapa().lock().unwrap_or_else(|p| p.into_inner());
    let nivel = peor_nivel(&ventanas);
    m.insert(
        provider.to_string(),
        Estado {
            provider: provider.to_string(),
            nivel,
            ventanas,
            detalle: None,
            actualizado_at: ahora(),
        },
    );
    drop(m);
    persistir();
}

/// Estado de un proveedor (o de todos, si `provider` es `None`).
pub fn estado(provider: Option<&str>) -> Vec<Estado> {
    let m = mapa().lock().unwrap_or_else(|p| p.into_inner());
    match provider {
        Some(p) => m
            .get(p)
            .cloned()
            .map(|e| vec![e])
            .unwrap_or_default(),
        None => m.values().cloned().collect(),
    }
}

/// El peor nivel entre todos los proveedores.
pub fn nivel_global() -> Nivel {
    let m = mapa().lock().unwrap_or_else(|p| p.into_inner());
    m.values()
        .map(|e| e.nivel)
        .max()
        .unwrap_or(Nivel::Normal)
}

/// ¿Este proveedor está en pausa (cuota agotada)?
pub fn en_pausa(provider: &str) -> bool {
    let m = mapa().lock().unwrap_or_else(|p| p.into_inner());
    m.get(provider).map(|e| e.nivel == Nivel::Pausa).unwrap_or(false)
}

/// ¿Estamos en modo ahorro (cuota casi agotada)?
pub fn en_ahorro(provider: &str) -> bool {
    let m = mapa().lock().unwrap_or_else(|p| p.into_inner());
    m.get(provider)
        .map(|e| e.nivel >= Nivel::Ahorro)
        .unwrap_or(false)
}

/// Segundos que faltan para el próximo reset conocido.
pub fn segundos_hasta_reset(provider: &str) -> Option<i64> {
    let m = mapa().lock().unwrap_or_else(|p| p.into_inner());
    let e = m.get(provider)?;
    let mut mejor: Option<u64> = None;
    for v in &e.ventanas {
        if let Some(r) = v.reset_at {
            mejor = Some(match mejor {
                Some(b) => b.min(r),
                None => r,
            });
        }
    }
    mejor.map(|r| r as i64 - ahora() as i64)
}

/// Texto corto para el agente (≤ 240 caracteres ⇒ ~60 tokens).
///
/// Se inyecta en la respuesta del chat para que el agente sepa lo que
/// queda **sin pedirlo**.
pub fn aviso_para_agente() -> Option<String> {
    let todos = estado(None);
    let mut peor: Option<Estado> = None;
    for e in todos {
        if e.nivel == Nivel::Normal {
            continue;
        }
        match &peor {
            None => peor = Some(e),
            Some(p) if e.nivel > p.nivel => peor = Some(e),
            _ => {}
        }
    }
    let e = peor?;
    let pct = e
        .ventanas
        .iter()
        .map(|v| v.pct_usado)
        .fold(0.0f64, f64::max);
    let reset = e
        .ventanas
        .iter()
        .filter_map(|v| v.reset_at)
        .min();
    let mut msg = format!(
        "QUOTA {} {}% ({})",
        e.provider,
        pct.round() as i64,
        e.nivel.as_str()
    );
    if let Some(r) = reset {
        let h = (r as i64 - ahora() as i64) as f64 / 3600.0;
        if h > 0.0 {
            msg.push_str(&format!("; reset en {h:.1} h"));
        }
    }
    match e.nivel {
        Nivel::Aviso => msg.push_str(" - guarda tu trabajo ya y no arranques tareas nuevas"),
        Nivel::Ahorro => {
            msg.push_str(" - modo AHORRO: usa el modelo local y NO abras tareas nuevas")
        }
        Nivel::Pausa => {
            msg.push_str(" - cuota AGOTADA: termina el paso actual, guarda checkpoint y espera")
        }
        Nivel::Normal => {}
    }
    Some(msg)
}

/// Snapshot **muy compacto** (mínimo gasto de tokens) para consultas.
///
/// Formato: `{"p":"minimax","n":"aviso","w":{"5h":82},"r":1.3,"m":"..."}`
/// ≈ 60-80 tokens, frente a los ~400 del snapshot completo.
pub fn snapshot_compacto() -> Value {
    snapshot_compacto_de(None)
}

/// Snapshot compacto de UN proveedor concreto.
pub fn snapshot_compacto_de(provider: Option<&str>) -> Value {
    let todos = match provider {
        Some(p) => estado(Some(p)),
        None => estado(None),
    };
    let mut out = serde_json::Map::new();
    let mut peor: Option<&Estado> = None;
    for e in &todos {
        if e.nivel == Nivel::Normal {
            continue;
        }
        match peor {
            None => peor = Some(e),
            Some(p) if e.nivel > p.nivel => peor = Some(e),
            _ => {}
        }
    }
    match peor {
        None => {
            out.insert("n".into(), json!("normal"));
        }
        Some(e) => {
            out.insert("p".into(), json!(e.provider));
            out.insert("n".into(), json!(e.nivel.as_str()));
            let mut w = serde_json::Map::new();
            for v in &e.ventanas {
                w.insert(v.nombre.clone(), json!(v.pct_usado.round() as i64));
            }
            out.insert("w".into(), Value::Object(w));
            if let Some(s) = segundos_hasta_reset(&e.provider) {
                out.insert("r_h".into(), json!((s as f64 / 3600.0 * 10.0).round() / 10.0));
            }
            if let Some(d) = &e.detalle {
                out.insert("m".into(), json!(d.chars().take(120).collect::<String>()));
            }
        }
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_429_de_quota() {
        let e = parsear_error_cuota("{\"error\":{\"message\":\"You have exceeded your quota for this organization\"}}");
        assert!(e.is_some());
    }

    #[test]
    fn detecta_resets_at() {
        let e = parsear_error_cuota("rate limit reached; resets at 1795130605").unwrap();
        assert_eq!(e.reset_at, Some(1795130605));
    }

    #[test]
    fn detecta_retry_after() {
        let e = parsear_error_cuota("429 Too Many Requests, retry after 42 seconds").unwrap();
        assert_eq!(e.retry_after, Some(42));
    }

    #[test]
    fn no_confunde_un_error_normal() {
        assert!(parsear_error_cuota("connection reset by peer").is_none());
        assert!(parsear_error_cuota("model not found").is_none());
    }

    #[test]
    fn los_umbros_son_jerarquicos() {
        assert_eq!(nivel_para(10.0), Nivel::Normal);
        assert_eq!(nivel_para(85.0), Nivel::Aviso);
        assert_eq!(nivel_para(92.0), Nivel::Ahorro);
        assert_eq!(nivel_para(99.0), Nivel::Pausa);
    }

    #[test]
    fn el_snapshot_compacto_es_pequeno() {
        let v = snapshot_compacto();
        let txt = serde_json::to_string(&v).unwrap();
        assert!(txt.len() < 400, "snapshot demasiado grande: {txt}");
    }

    #[test]
    fn registrar_error_deja_el_proveedor_en_pausa() {
        let p = "test-proveedor-pausa";
        registrar_error_cuota(p, 429, "quota exceeded for this key");
        assert!(en_pausa(p));
        let e = estado(Some(p)).pop().unwrap();
        assert_eq!(e.nivel, Nivel::Pausa);
        assert!(e.detalle.is_some());
    }
}
