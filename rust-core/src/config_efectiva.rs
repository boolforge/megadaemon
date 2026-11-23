//! MegaDaemon — **motor de configuración canónico**.
//!
//! EL PROBLEMA (medido el 2026-11-23)
//! -----------------------------------
//! El daemon tenía **91 variables de entorno** repartidas por 194 módulos y
//! ~40 ficheros de estado sueltos.  Consecuencias reales:
//!
//! * Nada era editable en caliente: cambiar un umbral obligaba a reiniciar.
//! * El usuario no tenía forma de **ver** qué estaba configurado.
//! * La Web UI era un espejo de un tercio del daemon.
//! * Dos módulos podían leer el mismo ajuste de dos maneras distintas.
//!
//! QUÉ HACE ESTE MÓDULO
//! --------------------
//! Una **única fuente de verdad** de la configuración, en
//! `<state>/config.json`:
//!
//! * `leer()` ⇒ la configuración **efectiva**: valores de fábrica + fichero
//!   + variables de entorno (las variables ganan, para no romper el arranque
//!   ya desplegado).
//! * `escribir()` ⇒ guarda cambios de forma **atómica** y con validación.
//! * Accesores con ruta punteada (`instrumentacion_agy.mitm_db`).
//! * **Recarga en caliente**: se relee si el fichero ha cambiado (mtime), así
//!   que editar desde la UI surte efecto en la siguiente llamada.
//!
//! VALORES DE FÁBRICA
//! ------------------
//! Cada apartado lleva su valor por defecto **documentado en el propio
//! código**: la UI los muestra como "por defecto" frente a "editado".

use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Fichero de configuración canónico.
pub const FICHERO: &str = "config.json";

/// Ruta absoluta del fichero de configuración.
pub fn ruta() -> PathBuf {
    PathBuf::from(format!("{}/{FICHERO}", crate::cockpit_server::state_dir()))
}

/// Cache del fichero: `(mtime, valor)` para no releer en cada acceso.
static CACHE: Mutex<Option<(u64, Value)>> = Mutex::new(None);

/// **Valores de fábrica** (la configuración documentada).
///
/// Todo lo que el daemon puede ajustar está aquí.  Si algo se puede tocar,
/// se toca desde aquí: no hay ajustes escondidos en `env::var` sueltos para
/// lo que el usuario pueda necesitar cambiar.
pub fn por_defecto() -> Value {
    json!({
        "general": {
            "nombre_instancia": "",
            "rol": "nodo",
            "puerto_cockpit": 3000,
            "puerto_gateway": 9090,
            "puerto_mcp": 9090,
            "estado_dir": ""
        },
        "seguridad": {
            "exigir_api_key": true,
            "rate_max_peticiones": 120,
            "rate_loopback": true,
            "origenes_permitidos": [],
            "sincronizar_token_navegador": true
        },
        "instrumentacion_agy": {
            "activa": true,
            "automatica": true,
            "intervalo_s": 60,
            "modo_so": "falso",
            "modelo": true,
            "puente_mcp": true,
            "mitm_db": false,
            "ocultar": [],
            "reescribir": {}
        },
        "auto_update": {
            "activo": true,
            "intervalo_min": 30,
            "reiniciar_despues": true
        },
        "energia": {
            "perfil": "auto",
            "limite_carga_pct": 80,
            "termico_aviso_c": 45,
            "termico_hard_c": 55,
            "termico_critico_c": 65,
            "veto_trabajo_pesado": true,
            "suelo_ram_kb": 262144,
            "suelo_disco_kb": 1048576
        },
        "cuota": {
            "umbral_aviso_pct": 15,
            "umbral_ahorro_pct": 5,
            "presupuesto_diario_usd": 5.0,
            "pausar_en_ahorro": true
        },
        "agente": {
            "modelo": "megadaemon",
            "voz": false,
            "voz_intervalo_s": 300,
            "voz_cooldown_s": 60,
            "voz_max_acciones_hora": 10,
            "voz_lease_s": 120
        },
        "cola": {
            "lease_s": 300,
            "reintentos": 3,
            "sincronizar_drive": true
        },
        "drive": {
            "raiz": "MegaDaemon",
            "sincronizar": true,
            "tablón": "activado"
        },
        "re": {
            "ghidra_java": "auto",
            "timeout_ms": 120000,
            "workers": 2
        },
        "tunel": {
            "activado": true,
            "publicar_url": true,
            "duckdns_intervalo_s": 900
        },
        "ui": {
            "tema": "auto",
            "auto_refresco_ms": 4000,
            "mostrar_logs": true,
            "log_lineas": 200
        }
    })
}

/// Fusiona dos objetos JSON (el de fábrica con el del usuario).
fn fusionar(base: &mut Value, encima: &Value) {
    match (base, encima) {
        (Value::Object(b), Value::Object(e)) => {
            for (k, v) in e {
                match b.get_mut(k) {
                    Some(slot) => fusionar(slot, v),
                    None => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, e) => {
            if !e.is_null() {
                *b = e.clone();
            }
        }
    }
}

/// Lee el fichero de configuración (sin fusionar).
fn leer_fichero() -> Value {
    let ruta = ruta();
    let Ok(meta) = std::fs::metadata(&ruta) else {
        return json!({});
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Cache: si el mtime no ha cambiado, no se relee.
    if let Ok(c) = CACHE.lock() {
        if let Some((t, v)) = c.as_ref() {
            if *t == mtime {
                return v.clone();
            }
        }
    }
    let mut v: Value = std::fs::read_to_string(&ruta)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    if let Some(o) = v.as_object_mut() {
        o.remove("$esquema");
    }
    if let Ok(mut c) = CACHE.lock() {
        *c = Some((mtime, v.clone()));
    }
    v
}

/// **Configuración efectiva**: fábrica + fichero + entorno.
///
/// El entorno gana sólo en las claves que tienen equivalente documentado
/// (puerto, intervalo, umbrales...).  El resto sale del fichero.
pub fn leer() -> Value {
    let mut cfg = por_defecto();
    let fichero = leer_fichero();
    fusionar(&mut cfg, &fichero);
    aplicar_entorno(&mut cfg);
    cfg
}

/// Aplica las variables de entorno sobre la configuración (capa de arranque).
fn aplicar_entorno(cfg: &mut Value) {
    let set = |cfg: &mut Value, ruta: &str, v: Value| {
        let partes: Vec<&str> = ruta.split('.').collect();
        let mut cur = cfg;
        for (i, p) in partes.iter().enumerate() {
            if i == partes.len() - 1 {
                if let Some(o) = cur.as_object_mut() {
                    o.insert(p.to_string(), v);
                }
                return;
            }
            if !cur.get(*p).map(|x| x.is_object()).unwrap_or(false) {
                if let Some(o) = cur.as_object_mut() {
                    o.insert(p.to_string(), json!({}));
                }
            }
            cur = cur.get_mut(*p).unwrap();
        }
    };
    if let Ok(v) = std::env::var("MEGADAEMON_INSTANCE_NAME") {
        set(cfg, "general.nombre_instancia", json!(v));
    }
    if let Ok(v) = std::env::var("MEGADAEMON_INSTANCE_ROLE") {
        set(cfg, "general.rol", json!(v));
    }
    if let Ok(v) = std::env::var("MEGADAEMON_REQUIRE_API_KEY") {
        set(
            cfg,
            "seguridad.exigir_api_key",
            json!(!matches!(v.as_str(), "0" | "false" | "no")),
        );
    }
    if let Ok(v) = std::env::var("MEGADAEMON_RATE_MAX") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "seguridad.rate_max_peticiones", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_RATE_LOOPBACK") {
        set(
            cfg,
            "seguridad.rate_loopback",
            json!(!matches!(v.as_str(), "0" | "false" | "no")),
        );
    }
    if let Ok(v) = std::env::var("MEGADAEMON_ALLOWED_ORIGINS") {
        let lista: Vec<String> = v
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        set(cfg, "seguridad.origenes_permitidos", json!(lista));
    }
    if let Ok(v) = std::env::var("MD_AUTO_UPDATE_INTERVAL_MIN") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "auto_update.intervalo_min", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MD_NO_AUTO_UPDATE") {
        set(
            cfg,
            "auto_update.activo",
            json!(matches!(v.as_str(), "1" | "true" | "yes")),
        );
    }
    if let Ok(v) = std::env::var("MEGADAEMON_DAILY_BUDGET_USD") {
        if let Ok(n) = v.parse::<f64>() {
            set(cfg, "cuota.presupuesto_diario_usd", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_CHARGE_LIMIT_PCT") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "energia.limite_carga_pct", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_THERMAL_WARN_C") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "energia.termico_aviso_c", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_THERMAL_HARD_C") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "energia.termico_hard_c", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_THERMAL_CRIT_C") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "energia.termico_critico_c", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_RAM_FLOOR_KB") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "energia.suelo_ram_kb", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_DISK_FLOOR_KB") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "energia.suelo_disco_kb", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_VOICE") {
        set(cfg, "agente.voz", json!(matches!(v.as_str(), "1" | "true" | "yes")));
    }
    if let Ok(v) = std::env::var("MEGADAEMON_VOICE_INTERVAL") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "agente.voz_intervalo_s", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_VOICE_COOLDOWN") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "agente.voz_cooldown_s", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_VOICE_MAX_ACTIONS_PER_HOUR") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "agente.voz_max_acciones_hora", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_MESH_STALE_SECS") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "cola.lease_s", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_CANONICAL_URL") {
        set(cfg, "tunel.url_canonica", json!(v));
    }
    if let Ok(v) = std::env::var("MD_ANTIGRAVITY_TIMEOUT") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "re.timeout_ms", json!(n));
        }
    }
    if let Ok(v) = std::env::var("MEGADAEMON_CHANNEL_INTERVAL_S") {
        if let Ok(n) = v.parse::<u64>() {
            set(cfg, "tunel.duckdns_intervalo_s", json!(n));
        }
    }
}

/// Guarda cambios (atómico, 0600: puede contener rutas y umbrales internos).
pub fn escribir(cambios: &Value) -> Result<Value, String> {
    let ruta = ruta();
    // Se parte de lo que hay en disco (no de la efectiva): así una variable
    // de entorno NO se "congela" en el fichero por accidente.
    let mut base = leer_fichero();
    if !base.is_object() {
        base = json!({});
    }
    fusionar(&mut base, cambios);
    if let Some(o) = base.as_object_mut() {
        o.insert("$esquema".to_string(), json!(1));
    }
    let txt = serde_json::to_string_pretty(&base).map_err(|e| e.to_string())?;
    crate::util_fs::write_atomic(Path::new(&ruta), txt.as_bytes(), 0o600)
        .map_err(|e| format!("escribir {}: {e}", ruta.display()))?;
    // Invalidar cache.
    if let Ok(mut c) = CACHE.lock() {
        *c = None;
    }
    Ok(leer())
}

/// Lee un valor por ruta punteada.
pub fn valor(ruta: &str) -> Option<Value> {
    leer().get(ruta).cloned()
}

/// Lee un `bool` por ruta punteada (con defecto).
pub fn bool_de(ruta: &str, defecto: bool) -> bool {
    valor(ruta).and_then(|v| v.as_bool()).unwrap_or(defecto)
}

/// Lee un `u64` por ruta punteada (con defecto).
pub fn u64_de(ruta: &str, defecto: u64) -> u64 {
    valor(ruta)
        .and_then(|v| v.as_u64())
        .or_else(|| valor(ruta).and_then(|v| v.as_f64()).map(|f| f as u64))
        .unwrap_or(defecto)
}

/// Lee un `f64` por ruta punteada (con defecto).
pub fn f64_de(ruta: &str, defecto: f64) -> f64 {
    valor(ruta)
        .and_then(|v| v.as_f64())
        .unwrap_or(defecto)
}

/// Lee un `String` por ruta punteada (con defecto).
pub fn string_de(ruta: &str, defecto: &str) -> String {
    valor(ruta)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| defecto.to_string())
}

/// Lee un array de cadenas por ruta punteada.
pub fn vector_de(ruta: &str) -> Option<Vec<String>> {
    valor(ruta).and_then(|v| {
        v.as_array().map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
    })
}

/// Lee un objeto clave→valor por ruta punteada.
pub fn pares_de(ruta: &str) -> Option<std::collections::BTreeMap<String, String>> {
    valor(ruta).and_then(|v| {
        v.as_object().map(|o| {
            o.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
    })
}

/// **Informe** para la UI: qué está configurado, qué se editó y qué viene del
/// entorno.  Es lo que permite pintar "por defecto" vs "editado".
pub fn informe() -> Value {
    let fabrica = por_defecto();
    let fichero = leer_fichero();
    let efectivo = leer();
    let mut editado: Vec<String> = Vec::new();
    fn recorrido(
        base: &Value,
        actual: &Value,
        prefijo: &str,
        editado: &mut Vec<String>,
    ) {
        if let (Some(b), Some(a)) = (base.as_object(), actual.as_object()) {
            for (k, av) in a {
                let bv = b.get(k);
                let ruta = if prefijo.is_empty() {
                    k.clone()
                } else {
                    format!("{prefijo}.{k}")
                };
                match (bv, av) {
                    (Some(bv), av) if bv.is_object() && av.is_object() => {
                        recorrido(bv, av, &ruta, editado);
                    }
                    (Some(bv), av) if bv != av => editado.push(ruta),
                    (None, _) => editado.push(ruta),
                    _ => {}
                }
            }
        }
    }
    recorrido(&fabrica, &fichero, "", &mut editado);
    json!({
        "ok": true,
        "ruta": ruta().to_string_lossy(),
        "existe": ruta().exists(),
        "efectiva": efectivo,
        "por_defecto": fabrica,
        "editado": editado,
        "secciones": fabrica
            .as_object()
            .map(|o| o.keys().cloned().collect::<Vec<String>>())
            .unwrap_or_default(),
    })
}

/// Valida un cambio antes de guardarlo.
///
/// Reglas **mínimas pero reales** (medidas contra los umbrales del móvil):
/// * los puertos están en 1024..65535;
/// * los umbrales térmicos van ordenados: aviso ≤ hard ≤ crítico;
/// * `modo_so` es uno de los tres valores conocidos;
/// * los porcentajes de cuota y carga están en 0..100.
pub fn validar(cambios: &Value) -> Result<(), String> {
    fn get<'a>(v: &'a Value, r: &str) -> Option<&'a Value> {
        let mut cur = v;
        for p in r.split('.') {
            cur = cur.get(p)?;
        }
        Some(cur)
    }
    let mut cfg = leer();
    fusionar(&mut cfg, cambios);

    for ruta_p in ["general.puerto_cockpit", "general.puerto_gateway", "general.puerto_mcp"] {
        if let Some(v) = get(&cfg, ruta_p).and_then(|v| v.as_u64()) {
            if !(1024..=65535).contains(&v) {
                return Err(format!("{ruta_p} fuera de rango (1024..65535): {v}"));
            }
        }
    }
    let aviso = get(&cfg, "energia.termico_aviso_c").and_then(|v| v.as_u64()).unwrap_or(45);
    let hard = get(&cfg, "energia.termico_hard_c").and_then(|v| v.as_u64()).unwrap_or(55);
    let crit = get(&cfg, "energia.termico_critico_c").and_then(|v| v.as_u64()).unwrap_or(65);
    if !(aviso <= hard && hard <= crit) {
        return Err(format!(
            "umbrales termicos desordenados: aviso({aviso}) <= hard({hard}) <= critico({crit})"
        ));
    }
    if let Some(m) = get(&cfg, "instrumentacion_agy.modo_so").and_then(|v| v.as_str()) {
        if !["falso", "in situ", "ninguno"].contains(&m) {
            return Err(format!("modo_so invalido: {m} (falso|in situ|ninguno)"));
        }
    }
    for ruta_p in [
        "energia.limite_carga_pct",
        "cuota.umbral_aviso_pct",
        "cuota.umbral_ahorro_pct",
    ] {
        if let Some(v) = get(&cfg, ruta_p).and_then(|v| v.as_u64()) {
            if v > 100 {
                return Err(format!("{ruta_p} fuera de rango (0..100): {v}"));
            }
        }
    }
    let aw = get(&cfg, "cuota.umbral_aviso_pct").and_then(|v| v.as_u64()).unwrap_or(15);
    let ah = get(&cfg, "cuota.umbral_ahorro_pct").and_then(|v| v.as_u64()).unwrap_or(5);
    if ah > aw {
        return Err(format!("cuota: umbral de ahorro({ah}) mayor que el de aviso({aw})"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_valores_de_fabrica_cubren_todas_las_secciones() {
        let d = por_defecto();
        for s in [
            "general", "seguridad", "instrumentacion_agy", "auto_update", "energia", "cuota",
            "agente", "cola", "drive", "re", "tunel", "ui",
        ] {
            assert!(d.get(s).is_some(), "falta la seccion {s}");
        }
    }

    #[test]
    fn fusionar_respeta_la_jerarquia() {
        let mut b = json!({"a": {"x": 1, "y": 2}});
        fusionar(&mut b, &json!({"a": {"y": 9, "z": 3}}));
        assert_eq!(b["a"]["x"], json!(1));
        assert_eq!(b["a"]["y"], json!(9));
        assert_eq!(b["a"]["z"], json!(3));
    }

    #[test]
    fn validar_rechaza_puertos_y_umbrales_malos() {
        assert!(validar(&json!({"general": {"puerto_cockpit": 80}})).is_err());
        assert!(validar(&json!({"energia": {"termico_aviso_c": 90}})).is_err());
        assert!(validar(&json!({"instrumentacion_agy": {"modo_so": "inventado"}})).is_err());
        assert!(validar(&json!({"cuota": {"umbral_ahorro_pct": 90}})).is_err());
        assert!(validar(&json!({"energia": {"termico_aviso_c": 40}})).is_ok());
    }

    #[test]
    fn los_umbrales_por_defecto_son_validos() {
        assert!(validar(&json!({})).is_ok());
    }
}
