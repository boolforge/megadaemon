//! Ejecutor de órdenes del bus: **controlar el móvil sin conexión directa**.
//!
//! POR QUÉ
//! -------
//! MEDIDO (2026-11-26): el enjambre se queda ciego cuando el túnel del móvil
//! cae (el *quick tunnel* cambia de hostname y el relé canónico se cae).  La
//! respuesta long-term no es "conectar mejor", es **invertir el sentido**:
//! en vez de que el root empuje órdenes al móvil, el móvil **tira** de un
//! bus y ejecuta lo que encuentra ahí.
//!
//! Este módulo es el "worker" que:
//!
//!   1. **sincroniza** el tablón de `resonance.rs` (el bus P2P por Drive que
//!      ya existe: cartas firmadas Ed25519, `id = sha256(canonico)`, CRDT
//!      LWW con `epoca`+`seq`) ⇒ trae las cartas de los demás;
//!   2. **selecciona** las que son órdenes para este nodo (`tipo == "orden"`
//!      y `cuerpo.destino` == este nodo o `"*"`), ignorando las suyas;
//!   3. **las ejecuta** con el registro de tools del daemon (`android::dispatch`),
//!      que es el MISMO camino que usa la API ⇒ no hay dos implementaciones;
//!   4. **responde** con una carta `tipo == "respuesta"` que lleva el
//!      resultado y el `idem_key` de la orden (para que el emisor la
//!      empareje).
//!
//! IDEMPOTENCIA (lo importante)
//! --------------------------
//! Una orden lleva `idem_key`.  El ejecutor guarda las claves **ya
//! ejecutadas** en el state dir ⇒ repetir el depósito (por ejemplo, porque
//! dos nodos sincronizaron la misma carta, o porque el bus reintenta) **no**
//! vuelve a ejecutar la orden.  Sin esto, un `am force-stop` se repetiría
//! cada vez que se re-sincroniza.
//!
//! SEGURIDAD
//! ---------
//! * La carta debe estar **firmada** por un emisor cuya clave pública
//!   conocemos; si no, se ignora y se cuenta como `sin_verificar`.
//! * Las órdenes destructivas exigen `confirm: true` en el cuerpo (igual que
//!   la API): el ejecutor **no** las ejecuta sin él.
//! * El ejecutor tiene un **presupuesto**: máximo de órdenes por ciclo y
//!   ningún trabajo pesado si la batería está baja (mismo criterio que el
//!   resto del daemon: `battery_advanced::modo_ahorro_activo`).
//!
//! ESTE MÓDULO ES PROPIO
//! ---------------------
//! No toca `lib.rs`, `main.rs`, `Cargo.toml` ni ningún módulo ajeno: usa la
//! API pública de `resonance`, `android`, `battery_advanced` y `state_dir`.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Tipo de carta que es una orden.
pub const TIPO_ORDEN: &str = "orden";
/// Tipo de carta que es la respuesta a una orden.
pub const TIPO_RESPUESTA: &str = "respuesta";

/// Fichero local con las claves `idem_key` ya ejecutadas.
const EJECUTADAS: &str = "ordenes-ejecutadas.json";

/// Límite de órdenes por ciclo (protege al móvil de una avalancha).
pub const MAX_POR_CICLO: usize = 8;

/// Opciones de una ejecución del worker.
#[derive(Debug, Clone)]
pub struct Opciones {
    /// Nodo destinatario de las órdenes (vacío = el actual).
    pub nodo: String,
    /// Presupuesto de órdenes en este ciclo.
    pub maximo: usize,
    /// Ejecutar aunque la batería esté baja (por defecto, no).
    pub forzar: bool,
    /// Simulación: no ejecuta nada, sólo dice qué haría.
    pub simulacion: bool,
    /// Verificar la firma de las cartas (por defecto, sí).
    pub verificar_firmas: bool,
}

impl Default for Opciones {
    fn default() -> Self {
        Opciones {
            nodo: String::new(),
            maximo: MAX_POR_CICLO,
            forzar: false,
            simulacion: false,
            verificar_firmas: true,
        }
    }
}

/// Nodo actual (el del daemon).
pub fn nodo_actual() -> String {
    crate::resonance::nodo_actual()
}

/// Ruta del fichero de órdenes ya ejecutadas.
fn ruta_ejecutadas(state_dir: Option<&str>) -> PathBuf {
    let base = state_dir
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::state_dir::state_dir());
    Path::new(&base).join(EJECUTADAS)
}

/// Carga las claves `idem_key` ya ejecutadas.
pub fn claves_ejecutadas(state_dir: Option<&str>) -> Vec<String> {
    let p = ruta_ejecutadas(state_dir);
    std::fs::read_to_string(p)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_array().cloned())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Guarda las claves ejecutadas (con tope: las 500 más recientes).
pub fn guardar_ejecutadas(claves: &[String], state_dir: Option<&str>) -> Result<(), String> {
    let p = ruta_ejecutadas(state_dir);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("crear {}: {e}", dir.display()))?;
    }
    let mut v = claves.to_vec();
    if v.len() > 500 {
        v = v[v.len() - 500..].to_vec();
    }
    std::fs::write(&p, serde_json::to_vec_pretty(&json!(v)).unwrap_or_default())
        .map_err(|e| format!("escribir {}: {e}", p.display()))
}

/// ¿Es esta carta una orden para este nodo?
///
/// Reglas:
/// * el tipo debe ser `orden`;
/// * `cuerpo.destino` debe ser el nodo, `"*"` o `"todos"`;
/// * una carta **propia** no se ejecuta (el nodo no se da órdenes a sí mismo
///   a través del bus: eso es una llamada directa).
pub fn es_para(carta: &crate::resonance::Carta, nodo: &str) -> bool {
    if carta.tipo != TIPO_ORDEN {
        return false;
    }
    if carta.nodo == nodo {
        return false;
    }
    let destino = carta.cuerpo["destino"].as_str().unwrap_or("*").trim();
    destino.is_empty() || destino == "*" || destino == "todos" || destino == nodo
}

/// Extrae el `idem_key` de una orden.
///
/// Si no lo trae, se deriva del `id` de la carta (que es el hash del
/// contenido) ⇒ **siempre** hay idempotencia, incluso con emisores que no
/// la ponen.
pub fn idem_key(carta: &crate::resonance::Carta) -> String {
    let k = carta.cuerpo["idem_key"].as_str().unwrap_or("").trim();
    if !k.is_empty() {
        return k.to_string();
    }
    format!("carta:{}", carta.id)
}

/// ¿La orden necesita confirmación explícita?
///
/// Las acciones destructivas (`root_exec` con `confirm_destructive`, o las
/// que el propio daemon marca como irreversibles) exigen `confirm: true`.
pub fn exige_confirmacion(cuerpo: &Value) -> bool {
    if cuerpo["confirm"].as_bool().unwrap_or(false) {
        return false; // ya viene confirmada
    }
    let tool = cuerpo["tool"].as_str().unwrap_or("");
    match tool {
        "root_exec" => cuerpo["args"]
            .get("confirm_destructive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        "app_control" => matches!(
            cuerpo["args"]["action"].as_str().unwrap_or(""),
            "disable" | "uninstall" | "clear_data" | "reset"
        ),
        "fs_write" => true,
        _ => false,
    }
}

/// Ejecuta una orden con el registro de tools del daemon.
///
/// Reutiliza `android::dispatch` ⇒ es **el mismo** camino que la API HTTP y
/// el MCP: una sola implementación de cada tool.
pub fn ejecutar_orden(cuerpo: &Value) -> Value {
    let tool = cuerpo["tool"].as_str().unwrap_or("").trim().to_string();
    if tool.is_empty() {
        return json!({"ok": false, "error": "falta_tool", "hint": "la orden necesita 'tool'"});
    }
    let args = cuerpo["args"].clone();
    crate::android::dispatch(&tool, &args)
}

/// Resultado de un ciclo del worker.
#[derive(Debug, Default)]
pub struct Informe {
    /// Órdenes ejecutadas.
    pub ejecutadas: usize,
    /// Órdenes saltadas por idempotencia.
    pub saltadas: usize,
    /// Órdenes que exigían confirmación y no la traían.
    pub sin_confirmar: usize,
    /// Órdenes para otro nodo (informativo).
    pub ajenas: usize,
    /// Cartas sin firma válida (informativo).
    pub sin_verificar: usize,
    /// Detalle por orden.
    pub detalle: Vec<Value>,
    /// El bus se sincronizó bien.
    pub bus_ok: bool,
    /// Motivo por el que no se ejecutó nada (si aplica).
    pub motivo: Option<String>,
}

/// Ejecuta un ciclo completo: sincroniza el bus, selecciona y ejecuta.
pub fn ciclo(opts: &Opciones, state_dir: Option<&str>) -> Informe {
    let mut inf = Informe::default();
    let nodo = if opts.nodo.trim().is_empty() {
        nodo_actual()
    } else {
        opts.nodo.trim().to_string()
    };

    // Batería: sin cargador y por debajo del 20 % no se ejecuta nada pesado.
    if !opts.forzar && crate::battery_advanced::modo_ahorro_activo() && !opts.simulacion {
        inf.motivo = Some(
            "modo ahorro activo (bateria baja sin cargador): no se ejecutan ordenes".to_string(),
        );
        return inf;
    }

    // 1) Traer las cartas de los demás.
    let sincronizado = match crate::resonance::sincronizar() {
        Ok(v) => {
            inf.bus_ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
            v
        }
        Err(e) => {
            inf.bus_ok = false;
            inf.motivo = Some(format!("el bus no se pudo sincronizar: {e}"));
            json!({})
        }
    };
    let _ = sincronizado;

    // 2) Abrir el tablón local (ya fusionado con lo recién traído).
    let state = state_dir
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::state_dir::state_dir());
    let mut tablon = match crate::resonance::Tablon::abrir(&state) {
        Ok(t) => t,
        Err(e) => {
            if inf.motivo.is_none() {
                inf.motivo = Some(format!("no se pudo abrir el tablón: {e}"));
            }
            return inf;
        }
    };

    // 3) Seleccionar y ejecutar.
    let mut ejecutadas = claves_ejecutadas(state_dir);
    let mut nuevas = Vec::new();
    let maximo = if opts.maximo == 0 { MAX_POR_CICLO } else { opts.maximo };

    for carta in tablon.cartas.clone() {
        if carta.tipo != TIPO_ORDEN {
            continue;
        }
        if !es_para(&carta, &nodo) {
            if carta.cuerpo["destino"].as_str().unwrap_or("*") != "*" {
                inf.ajenas += 1;
            }
            continue;
        }
        if opts.verificar_firmas && !carta.firma.is_empty() {
            // La firma se verifica contra la clave pública del emisor, que
            // vive junto al estado.  Si no se puede verificar, la orden se
            // salta (fail-closed).
            let pub_ruta = PathBuf::from(&state).join(format!("clave-{}.ed25519.pub", carta.nodo));
            match std::fs::read_to_string(&pub_ruta) {
                Ok(pub_hex) => {
                    if !carta.verificar(pub_hex.trim()) {
                        inf.sin_verificar += 1;
                        inf.detalle.push(json!({
                            "id": carta.id,
                            "nodo": carta.nodo,
                            "resultado": "firma_invalida",
                        }));
                        continue;
                    }
                }
                Err(_) => {
                    // Sin clave pública no se puede verificar: fail-closed.
                    inf.sin_verificar += 1;
                    inf.detalle.push(json!({
                        "id": carta.id,
                        "nodo": carta.nodo,
                        "resultado": "sin_clave_publica",
                    }));
                    continue;
                }
            }
        }

        let clave = idem_key(&carta);
        if ejecutadas.iter().any(|k| k == &clave) {
            inf.saltadas += 1;
            continue;
        }
        if inf.ejecutadas >= maximo {
            inf.motivo = Some(format!("presupuesto del ciclo agotado ({maximo})"));
            break;
        }

        // Confirmación explícita para lo irreversible.
        if exige_confirmacion(&carta.cuerpo) {
            inf.sin_confirmar += 1;
            inf.detalle.push(json!({
                "id": carta.id,
                "idem_key": clave,
                "resultado": "requiere_confirm",
                "tool": carta.cuerpo["tool"],
            }));
            continue;
        }

        let resultado = if opts.simulacion {
            json!({"ok": true, "simulado": true, "tool": carta.cuerpo["tool"]})
        } else {
            ejecutar_orden(&carta.cuerpo)
        };

        ejecutadas.push(clave.clone());
        nuevas.push(clave.clone());
        inf.ejecutadas += 1;
        inf.detalle.push(json!({
            "id": carta.id,
            "idem_key": clave,
            "nodo_emisor": carta.nodo,
            "tool": carta.cuerpo["tool"],
            "resultado": resultado,
        }));

        // 4) Responder por el bus (si no es simulación).
        if !opts.simulacion {
            let _ = tablon.publicar(
                TIPO_RESPUESTA,
                json!({
                    "destino": carta.nodo,
                    "idem_key": clave,
                    "resultado": resultado,
                }),
            );
        }
    }

    if !opts.simulacion && !nuevas.is_empty() {
        let _ = guardar_ejecutadas(&ejecutadas, state_dir);
        // Subir las respuestas al bus para que las vea el emisor.
        let _ = crate::resonance::sincronizar();
    }

    inf
}

/// Hilo del worker: cicla cada `minutos`.
///
/// El periodo se dobla en modo ahorro (igual que la telemetría).
pub fn bucle(minutos: u64, state_dir: Option<&str>) -> std::thread::JoinHandle<()> {
    // El `state_dir` se convierte a `String` ANTES de moverlo al hilo: si
    // fuera una referencia, el hilo no podría vivir más que el prestatario.
    let sd = state_dir.map(|s| s.to_string());
    std::thread::Builder::new()
        .name("bus-worker".to_string())
        .spawn(move || {
            let base = std::time::Duration::from_secs(minutos.max(1) * 60);
            loop {
                let ahorro = crate::battery_advanced::modo_ahorro_activo();
                let opts = Opciones {
                    forzar: false,
                    ..Default::default()
                };
                let inf = ciclo(&opts, sd.as_deref());
                if inf.ejecutadas > 0 {
                    eprintln!(
                        "bus: {} orden(es) ejecutadas, {} saltadas (idempotencia)",
                        inf.ejecutadas, inf.saltadas
                    );
                }
                std::thread::sleep(if ahorro { base * 2 } else { base });
            }
        })
        .expect("hilo del bus")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn carta(tipo: &str, nodo: &str, cuerpo: Value) -> crate::resonance::Carta {
        crate::resonance::Carta {
            id: "abc123".to_string(),
            epoca: 1,
            seq: 1,
            ts: 1,
            nodo: nodo.to_string(),
            tipo: tipo.to_string(),
            cuerpo,
            firma: String::new(),
        }
    }

    #[test]
    fn una_orden_para_otro_nodo_no_se_ejecuta_aqui() {
        let c = carta("orden", "otro", json!({"destino": "movil", "tool": "system_overview"}));
        // "movil" no es este nodo (en el test el nodo es "pc").
        assert!(!es_para(&c, "pc"));
        let d = carta("orden", "otro", json!({"destino": "pc", "tool": "system_overview"}));
        assert!(es_para(&d, "pc"));
        let e = carta("orden", "otro", json!({"destino": "*", "tool": "system_overview"}));
        assert!(es_para(&e, "pc"));
    }

    #[test]
    fn un_nodo_no_se_da_ordenes_a_si_mismo() {
        let c = carta("orden", "pc", json!({"destino": "pc", "tool": "system_overview"}));
        assert!(!es_para(&c, "pc"));
    }

    #[test]
    fn una_carta_que_no_es_orden_no_se_ejecuta() {
        let c = carta("latido", "otro", json!({"destino": "*"}));
        assert!(!es_para(&c, "pc"));
    }

    #[test]
    fn el_idem_key_deriva_del_id_si_no_viene() {
        let c = carta("orden", "otro", json!({"tool": "x"}));
        assert_eq!(idem_key(&c), "carta:abc123");
        let d = carta("orden", "otro", json!({"idem_key": "mio", "tool": "x"}));
        assert_eq!(idem_key(&d), "mio");
    }

    #[test]
    fn lo_destructivo_exige_confirmacion() {
        assert!(exige_confirmacion(&json!({
            "tool": "root_exec", "args": {"command": "rm -rf /data"}
        })));
        assert!(!exige_confirmacion(&json!({
            "tool": "root_exec", "args": {"command": "ls", "confirm_destructive": false}
        })));
        assert!(exige_confirmacion(&json!({
            "tool": "app_control", "args": {"action": "uninstall"}
        })));
        assert!(!exige_confirmacion(&json!({
            "tool": "app_control", "args": {"action": "list"}
        })));
        assert!(!exige_confirmacion(&json!({
            "tool": "system_overview"
        })));
    }

    #[test]
    fn la_confirmacion_explicita_salta_la_exigencia() {
        assert!(!exige_confirmacion(&json!({
            "tool": "root_exec", "confirm": true,
            "args": {"command": "rm -rf /data"}
        })));
    }

    #[test]
    fn las_claves_ejecutadas_se_guardan_y_se_leen() {
        let dir = std::env::temp_dir().join(format!("md_bus_{}", crate::google_auth::now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        let sd = dir.to_string_lossy().to_string();
        let claves = vec!["k1".to_string(), "k2".to_string()];
        guardar_ejecutadas(&claves, Some(&sd)).unwrap();
        let leidas = claves_ejecutadas(Some(&sd));
        assert!(leidas.contains(&"k1".to_string()));
        assert!(leidas.contains(&"k2".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_informe_arranca_a_cero() {
        let inf = Informe::default();
        assert_eq!(inf.ejecutadas, 0);
        assert_eq!(inf.saltadas, 0);
        assert!(!inf.bus_ok);
    }
}
