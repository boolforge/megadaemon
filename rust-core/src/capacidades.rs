//! **Fachada de las capacidades propias**: un solo punto de entrada.
//!
//! MEDIDO (2026-11-28): las capacidades escritas con el procedimiento
//! AST → Rust de `docs/EXTENSIONES_PROPIETARIAS.md` §4 y los patrones
//! CLI/Rust cerrados tienen **módulo propio** (`cola_mensajes.rs`,
//! `subagentes.rs`, `mcp_control.rs`, `model_options.rs`, `contexto.rs`,
//! `message_tree.rs`), pero sin un punto donde llamarlos: eran código
//! inalcanzable.  Este módulo es ese punto.
//!
//! QUÉ HACE
//! --------
//!
//! * **Un** `accion` con forma `familia.opcion` y un objeto de `params`
//!   libre, igual que las tools del repo.
//! * La cola de mensajes y los subagentes **se persisten** en
//!   `<state_dir>/capacidades.json` (escritura atómica: `.tmp` + rename),
//!   así que sobreviven a un reinicio del daemon.
//! * Lo que es **puro** (opciones de modelo, construcción del contexto)
//!   no toca disco.
//! * Cada respuesta dice de dónde sale el estado (`persisted: true/false`)
//!   y qué parte **no** está persistida todavía, para no fingir.
//!
//! LO QUE NO ESTÁ MEDIDO
//! ---------------------
//!
//! * El **opt-in mutuo** de los subagentes lo aplica el cliente ACP al
//!   declarar sus capacidades en el `initialize`; aquí se usan las que
//!   llegan en `params.capacidades` y, si no llegan, se suppose que el
//!   llamador las tiene todas (la respuesta lo dice: `opt_in_supuesto`).
//! * El estado de la cola y de los subagentes **no** se comparte con el
//!   bus de eventos (`board.rs`): la cola vive en su fichero.
//! * El estado del **revert** (`message_tree.rs`) es en memoria: cada
//!   llamada reconstruye el bosque desde los mensajes que se le pasan.

use serde_json::{json, Value};

use crate::cola_mensajes::Cola;
use crate::subagentes::{CapacidadesCliente, Subagentes};

/// Ruta del fichero donde vive el estado de las capacidades.
pub fn ruta_estado() -> String {
    format!("{}/capacidades.json", crate::state_dir::state_dir())
}

/// Carga el estado persistido.
///
/// Si el fichero **no está**, se empieza vacío (es el primer arranque).
/// Si está pero **no se puede leer**, se devuelve el error: tragarse un
/// estado corrupto sería perder la cola en silencio.
pub fn cargar() -> Result<(Cola, Subagentes), String> {
    let bruto = match std::fs::read_to_string(ruta_estado()) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((
                Cola::nueva(),
                Subagentes::nuevo(&crate::state_dir::state_dir()),
            ))
        }
        Err(e) => return Err(format!("capacidades: no se pudo leer el estado: {e}")),
    };
    let v: Value =
        serde_json::from_str(&bruto).map_err(|e| format!("capacidades: estado ilegible: {e}"))?;
    let cola = match v.get("cola") {
        Some(c) => Cola::desde_json(c)?,
        None => Cola::nueva(),
    };
    let subs = match v.get("subagentes") {
        Some(s) => Subagentes::desde_json(s)?,
        None => Subagentes::nuevo(&crate::state_dir::state_dir()),
    };
    Ok((cola, subs))
}

/// Guarda el estado (escritura **atómica**: `.tmp` + `rename`).
pub fn guardar(cola: &Cola, subs: &Subagentes) -> Result<(), String> {
    let ruta = ruta_estado();
    let dir = crate::state_dir::state_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("capacidades: no se pudo crear {dir}: {e}"))?;
    let texto = serde_json::to_string_pretty(&json!({
        "version": 1,
        "cola": cola.estado_json(),
        "subagentes": subs.estado_json(),
    }))
    .map_err(|e| format!("capacidades: estado ilegible: {e}"))?;
    let tmp = format!("{ruta}.tmp");
    std::fs::write(&tmp, texto)
        .map_err(|e| format!("capacidades: no se pudo escribir {tmp}: {e}"))?;
    std::fs::rename(&tmp, &ruta)
        .map_err(|e| format!("capacidades: no se pudo renombrar a {ruta}: {e}"))
}

/// Normaliza los nombres de los campos: las tools del repo usan castellano
/// ("texto", "prioridad") y las capacidades usan el nombre del API
/// ("text", "priority").  Si viene el canonico, manda el canonico.
fn normaliza(params: &mut Value) {
    let Some(o) = params.as_object_mut() else {
        return;
    };
    for (de, a) in [
        ("texto", "text"),
        ("prioridad", "priority"),
        ("motivo", "reason"),
        ("tarea", "task"),
        ("presupuesto_tokens", "budget_tokens"),
        ("limite", "limit"),
        ("estado", "state"),
    ] {
        if let Some(v) = o.get(de).cloned() {
            o.entry(a.to_string()).or_insert(v);
        }
    }
}

/// Las capacidades que el cliente declara (opt-in mutuo).
fn cliente(params: &Value) -> (CapacidadesCliente, bool) {
    let mut c = CapacidadesCliente::nueva();
    let declaradas = params.get("capacidades").and_then(|v| v.as_array());
    match declaradas {
        Some(lista) => {
            for x in lista {
                if let Some(s) = x.as_str() {
                    c.declara(s);
                }
            }
            (c, false)
        }
        None => {
            // Sin declaración explícita se supone que el llamador tiene
            // todas: la respuesta lo dice (`opt_in_supuesto`).
            for cap in crate::subagentes::capacidades_agente() {
                c.declara(cap);
            }
            (c, true)
        }
    }
}

/// Los proveedores del daemon, en el formato del catálogo de modelos.
fn proveedores_del_daemon() -> Vec<crate::model_options::DaemonProvider> {
    let vault = crate::providers::load_vault();
    vault
        .providers
        .iter()
        .filter(|p| p.enabled)
        .map(|p| crate::model_options::DaemonProvider {
            id: p.id.clone(),
            name: p.name.clone(),
            vendor: p.vendor.clone(),
            endpoint: p.endpoint.clone(),
            model: p.model.clone(),
            enabled: true,
        })
        .collect()
}

/// Opciones de modelo (puro: no toca disco).
fn modelos(sub: &str, params: &Value) -> Result<Value, String> {
    use crate::model_options as mo;
    match sub {
        "listar" => {
            let opciones = mo::list_options(&proveedores_del_daemon());
            Ok(json!({
                "ok": true,
                "opciones": mo::grouped_to_json(&opciones),
                "total": opciones.len(),
            }))
        }
        "enriquecer" => {
            let daemon = mo::list_options(&proveedores_del_daemon());
            let catalogo = params.get("catalogo").cloned().unwrap_or(Value::Null);
            let cliente = mo::parse_client_catalog(&catalogo).map_err(|e| e.to_string())?;
            let mezcla = mo::enrich(&daemon, &cliente);
            Ok(json!({
                "ok": true,
                "opciones": mo::grouped_to_json(&mezcla),
                "del_cliente": cliente.len(),
                "total": mezcla.len(),
            }))
        }
        "filtrar" => {
            let proveedor = params
                .get("proveedor")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let daemon = mo::list_options(&proveedores_del_daemon());
            let lista = mo::filter_by_provider(&daemon, proveedor);
            Ok(json!({
                "ok": true,
                "opciones": mo::grouped_to_json(&lista),
                "total": lista.len(),
            }))
        }
        "default" | "defecto" => {
            let daemon = mo::list_options(&proveedores_del_daemon());
            match mo::default_option(&daemon) {
                Some(o) => Ok(json!({ "ok": true, "opcion": mo::option_to_json(&o) })),
                None => Ok(json!({ "ok": true, "opcion": Value::Null })),
            }
        }
        _ => Err(format!("modelos: opcion desconocida: {sub}")),
    }
}

/// Construcción del contexto (puro: no habla con el modelo).
fn contexto(sub: &str, params: &Value) -> Result<Value, String> {
    if sub != "construir" {
        return Err(format!("contexto: opcion desconocida: {sub}"));
    }
    let texto = params
        .get("texto")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let estado = crate::contexto::EstadoContexto {
        sistema: params
            .get("sistema")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        reglas: cadena_lista(params.get("reglas")),
        memoria: cadena_lista(params.get("memoria")),
        ficheros: Vec::new(),
        historial: crate::compaction_epoch::Historial::nuevo("cap"),
        proveedores: Vec::new(),
        presupuesto_chars: params
            .get("presupuesto_chars")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize),
    };
    let entrada = crate::contexto::Entrada::nueva(texto);
    let ctx = crate::contexto::construir_contexto(&estado, &entrada);
    let m = &ctx.metadatos;
    Ok(json!({
        "ok": true,
        "prompt": ctx.to_prompt(),
        "bloques": ctx.bloques.len(),
        "metadatos": {
            "bloques": m.bloques,
            "chars_totales": m.chars_totales,
            "chars_por_clase": m.chars_por_clase.iter()
                .map(|(k, v)| (k.etiqueta().to_string(), json!(v)))
                .collect::<serde_json::Map<String, Value>>(),
            "presupuesto_chars": m.presupuesto_chars,
            "cabe": m.cabe,
            "recortado": m.recortado,
            "epoch": m.epoch,
            "mensajes_vista_viva": m.mensajes_vista_viva,
            "mensajes_visibles": m.mensajes_visibles,
            "total_inicial_chars": m.total_inicial_chars,
            "recortes": m.recortes.iter().map(|r| json!({
                "bloque": r.clase.etiqueta(),
                "chars_ahorrados": r.chars_ahorrados,
                "unidades_eliminadas": r.unidades_eliminadas,
                "bloque_eliminado": r.bloque_eliminado,
                "motivo": r.motivo,
            })).collect::<Vec<Value>>(),
        },
    }))
}

/// Una lista de cadenas (acepta `["a","b"]` o `"a\nb"`).
fn cadena_lista(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect(),
        Some(Value::String(s)) if !s.trim().is_empty() => {
            s.lines().map(|l| l.to_string()).collect()
        }
        _ => Vec::new(),
    }
}

/// El bosque de la conversación y el revert por pasos (en memoria).
fn revert(sub: &str, params: &Value) -> Result<Value, String> {
    use crate::message_tree as mt;
    let sesion = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("cap")
        .to_string();
    let mut estado = mt::Estado::nuevo(sesion.clone());
    let mensajes = params.get("mensajes").and_then(|v| v.as_array());
    if let Some(lista) = mensajes {
        for m in lista {
            let rol = m.get("rol").and_then(|v| v.as_str()).unwrap_or("user");
            let texto = m.get("texto").and_then(|v| v.as_str()).unwrap_or("");
            let _ = estado.anadir_mensaje(rol, texto);
        }
    }
    match sub {
        "pasos" => {
            let objetivo = params
                .get("objetivo")
                .and_then(|v| v.as_i64())
                .unwrap_or_default();
            let pasos = mt::pasos_revert(&estado, objetivo);
            Ok(json!({
                "ok": true,
                "pasos": pasos.iter().map(|p| json!({
                    "indice": p.indice,
                    "nodo_id": p.nodo_id,
                    "etiqueta": p.etiqueta,
                    "distancia": p.distancia,
                })).collect::<Vec<_>>(),
                "total": pasos.len(),
            }))
        }
        "ejecutar" => {
            let indice = params.get("indice").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let nuevo = mt::aplicar_paso(&estado, indice)?;
            Ok(json!({
                "ok": true,
                "vista_viva": nuevo.vista_viva().len(),
                "todos": nuevo.todos_los_mensajes().len(),
            }))
        }
        _ => Err(format!("revert: opcion desconocida: {sub}")),
    }
}

/// **Despacha** una acción de las capacidades propias.
///
/// Entrada: `{"accion": "cola.listar", ...params}`.
/// Salida: un objeto JSON con `ok` y el resultado (o el error).
pub fn despachar(args: &Value) -> Value {
    let accion = args
        .get("accion")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let mut params = args.clone();
    if let Some(o) = params.as_object_mut() {
        o.remove("accion");
    }
    if accion.is_empty() {
        return json!({
            "ok": false,
            "error": "falta 'accion'",
            "acciones": acciones(),
        });
    }
    normaliza(&mut params);
    let (mut cola, mut subs) = match cargar() {
        Ok(v) => v,
        Err(e) => {
            return json!({ "ok": false, "accion": accion, "error": e, "acciones": acciones() })
        }
    };
    let ahora = crate::cola_mensajes::ahora_s();
    let (cliente, supuesto) = cliente(&params);
    let familia = accion.split('.').next().unwrap_or("").to_string();
    let sub = accion
        .split_once('.')
        .map(|(_, s)| s.to_string())
        .unwrap_or_default();

    let resultado: Result<Value, String> = match familia.as_str() {
        "cola" => {
            // La cola exige que la sesion exista: se registra la primera vez.
            if let Some(s) = params.get("session_id").and_then(|v| v.as_str()) {
                cola.registrar_sesion(s);
            }
            crate::cola_mensajes::opera(&mut cola, &sub, &params, ahora).map_err(|e| e.mensaje())
        }
        "subagente" => crate::subagentes::opera(&mut subs, &cliente, &sub, &params, ahora)
            .map_err(|e| e.mensaje()),
        "modelos" => modelos(&sub, &params),
        "contexto" => contexto(&sub, &params),
        "revert" => revert(&sub, &params),
        _ => Err(format!("accion desconocida: {accion}")),
    };

    // Sólo se guarda lo que tiene estado (cola y subagentes).
    let persistente = matches!(familia.as_str(), "cola" | "subagente");
    let guardado = if persistente {
        guardar(&cola, &subs).err()
    } else {
        None
    };

    match resultado {
        Ok(mut v) => {
            if let Some(o) = v.as_object_mut() {
                o.insert("accion".to_string(), json!(accion));
                o.insert("persistido".to_string(), json!(persistente));
                if !supuesto {
                    o.insert("opt_in_supuesto".to_string(), json!(false));
                }
            }
            if let Some(e) = guardado {
                if let Some(o) = v.as_object_mut() {
                    o.insert("aviso_persistencia".to_string(), json!(e));
                }
            }
            v
        }
        Err(e) => json!({
            "ok": false,
            "accion": accion,
            "error": e,
            "acciones": acciones(),
        }),
    }
}

/// Las acciones que esta fachada entiende.
pub fn acciones() -> Vec<&'static str> {
    vec![
        "cola.listar",
        "cola.encolar",
        "cola.actualizar",
        "cola.borrar",
        "cola.redirigir",
        "subagente.listar",
        "subagente.obtener",
        "subagente.foreground",
        "subagente.background",
        "subagente.cancelar",
        "modelos.listar",
        "modelos.enriquecer",
        "modelos.filtrar",
        "modelos.default",
        "contexto.construir",
        "revert.pasos",
        "revert.ejecutar",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_accion_inexistente_devuelve_error_tipado() {
        let r = despachar(&json!({ "accion": "nada.de.nada" }));
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().contains("accion desconocida"));
        assert!(r["acciones"].as_array().unwrap().len() > 5);
    }

    #[test]
    fn sin_accion_devuelve_la_lista() {
        let r = despachar(&json!({}));
        assert_eq!(r["ok"], false);
        assert!(r["acciones"].as_array().unwrap().len() > 5);
    }

    #[test]
    fn la_cola_se_persiste_y_vuelve() {
        // 1) Encolar.
        let r = despachar(&json!({
            "accion": "cola.encolar",
            "session_id": "s1",
            "texto": "hola",
        }));
        assert_eq!(r["ok"], true, "{}", r);
        assert_eq!(r["persistido"], true);

        // 2) La cola guardada se lee al volver.
        let r = despachar(&json!({ "accion": "cola.listar", "session_id": "s1" }));
        assert_eq!(r["ok"], true, "{}", r);
        assert_eq!(r["total"], 1, "{}", r);
        let mensajes = r["messages_in_queued"].as_array().unwrap();
        assert_eq!(mensajes[0]["text"], "hola");
    }

    #[test]
    fn el_contexto_se_construye_sin_red() {
        let r = despachar(&json!({
            "accion": "contexto.construir",
            "texto": "lee el fichero",
            "sistema": "eres un agente",
        }));
        assert_eq!(r["ok"], true, "{}", r);
        let prompt = r["prompt"].as_str().unwrap();
        assert!(prompt.contains("lee el fichero"), "{prompt}");
        assert!(prompt.contains("eres un agente"), "{prompt}");
    }

    #[test]
    fn los_pasos_de_revert_se_listan() {
        let r = despachar(&json!({
            "accion": "revert.pasos",
            "session_id": "s1",
            "mensajes": [
                {"rol": "user", "texto": "hola"},
                {"rol": "assistant", "texto": "buenas"},
            ],
        }));
        assert_eq!(r["ok"], true, "{}", r);
        assert!(r["pasos"].as_array().is_some());
    }
}
