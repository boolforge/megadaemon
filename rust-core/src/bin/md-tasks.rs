//! `md-tasks` — la cola compartida de MegaDaemon.
//!
//! Una sola cola para TODA la flota: el móvil, el PC y los agentes.
//! Cada agente **busca** trabajo, lo toma con un lease, lo ejecuta y lo
//! devuelve con su evidencia.  Si el nodo se queda sin cuota o muere, la
//! tarea NO se pierde: el lease caduca o se pausa y otro nodo la retoma.
//!
//! ```text
//! md-tasks list [--estado pending|running|paused|done]
//! md-tasks submit --kind K --prioridad N [--json PAYLOAD]
//! md-tasks claim --nodo NODO [--max N] [--lease S]
//! md-tasks lease --nodo NODO [--s S]     # renueva el lease
//! md-tasks migrate --desde A --hacia B   # handoff a otro nodo
//! md-tasks pause --nodo N --until EPOCH --motivo M
//! md-tasks sync                           # sincroniza con Drive (todos los nodos)
//! md-tasks export | md-tasks import <json>
//! ```

use megadaemon_core::task_scheduler as ts;
use serde_json::{json, Value};

fn ayuda() {
    println!(
        "md-tasks — cola compartida de tareas de MegaDaemon

  list [--estado S]              lista tareas
  submit --kind K [--prioridad N] [--json PAYLOAD]
  claim --nodo N [--max N] [--lease S]   toma tareas con lease
  lease --nodo N [--s S]         renueva el lease de las tareas en curso
  migrate --desde A --hacia B    pasa el trabajo de un nodo a otro
  pause --nodo N --until E --motivo M     pausa por cuota/red
  sync                           sincroniza la cola con Drive (todos los nodos)
  export | import <json>        importa/exporta la cola

  Cuota: si el proveedor esta en 'pausa', las tareas nuevas NO se toman."
    );
}

fn arg(args: &[String], nombre: &str) -> Option<String> {
    args.iter()
        .position(|a| a == nombre)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        ayuda();
        return;
    }
    let sub = args[0].clone();
    let resto: Vec<String> = args[1..].to_vec();

    let v: Value = match sub.as_str() {
        "list" => {
            let t = ts::list();
            let filtro = arg(&resto, "--estado").unwrap_or_default();
            let items: Vec<Value> = t
                .iter()
                .filter(|x| filtro.is_empty() || x.state.as_str() == filtro)
                .map(|x| {
                    json!({
                        "id": x.id, "kind": x.kind, "state": x.state.as_str(),
                        "priority": x.priority, "node": x.node,
                        "attempts": x.attempts,
                        "resume_after": x.resume_after,
                        "motivo_pausa": x.motivo_pausa,
                        "error": x.error,
                    })
                })
                .collect();
            json!({ "ok": true, "count": items.len(), "tareas": items })
        }
        "submit" => {
            let kind = arg(&resto, "--kind").unwrap_or_else(|| "generica".to_string());
            let prio = arg(&resto, "--prioridad")
                .and_then(|p| p.parse::<u8>().ok())
                .unwrap_or(5);
            let payload: Value = arg(&resto, "--json")
                .and_then(|p| serde_json::from_str(&p).ok())
                .unwrap_or(Value::Null);
            let id = ts::submit(&kind, payload, prio, vec![], Default::default());
            json!({ "ok": true, "id": id })
        }
        "claim" => {
            let nodo = arg(&resto, "--nodo").unwrap_or_else(|| "pc".to_string());
            let max = arg(&resto, "--max")
                .and_then(|p| p.parse::<usize>().ok())
                .unwrap_or(1);
            let lease = arg(&resto, "--lease")
                .and_then(|p| p.parse::<u64>().ok())
                .unwrap_or(900);
            // Si la cuota esta en pausa, NO se toma trabajo nuevo: se
            // avisa y se deja la cola intacta.
            if megadaemon_core::quota_guard::nivel_global()
                == megadaemon_core::quota_guard::Nivel::Pausa
            {
                json!({
                    "ok": false,
                    "motivo": "cuota en pausa: no se toman tareas nuevas",
                    "aviso": megadaemon_core::quota_guard::aviso_para_agente(),
                })
            } else {
                let ids = ts::repartir_por_nodo(&nodo, max, lease);
                json!({ "ok": true, "nodo": nodo, "tomadas": ids })
            }
        }
        "lease" => {
            let nodo = arg(&resto, "--nodo").unwrap_or_else(|| "pc".to_string());
            let s = arg(&resto, "--s")
                .and_then(|p| p.parse::<u64>().ok())
                .unwrap_or(900);
            json!({ "ok": true, "renovadas": ts::renovar_lease(&nodo, s) })
        }
        "migrate" => {
            let desde = arg(&resto, "--desde").unwrap_or_default();
            let hacia = arg(&resto, "--hacia").unwrap_or_default();
            if desde.is_empty() || hacia.is_empty() {
                json!({ "ok": false, "error": "hacen falta --desde y --hacia" })
            } else {
                json!({ "ok": true, "migradas": ts::migrar_tareas(&desde, &hacia) })
            }
        }
        "pause" => {
            let nodo = arg(&resto, "--nodo").unwrap_or_default();
            let until = arg(&resto, "--until").and_then(|p| p.parse::<u64>().ok());
            let motivo = arg(&resto, "--motivo").unwrap_or_else(|| "cuota".to_string());
            json!({ "ok": true, "pausadas": ts::pausar_por_cuota(&nodo, until, &motivo) })
        }
        "sync" => match megadaemon_core::task_sync::sincronizar_drive() {
            Ok(r) => r,
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "export" => json!({ "ok": true, "tasks": ts::export_tasks("pc") }),
        "import" => {
            let txt = std::fs::read_to_string("tasks.json").unwrap_or_default();
            match serde_json::from_str::<Value>(&txt) {
                Ok(v) => json!({ "ok": true, "importadas": ts::import_tasks(&v) }),
                Err(e) => json!({ "ok": false, "error": format!("JSON invalido: {e}") }),
            }
        }
        _ => {
            ayuda();
            return;
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".to_string())
    );
}
