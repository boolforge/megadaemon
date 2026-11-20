//! Meta-agente: una petición en lenguaje natural → tarea → servicio → acción.
//!
//! El bus de servicios (`service_bus`) ya sabe **elegir** un servicio dado un
//! `TaskType`; lo que faltaba era el paso anterior: entender **qué** pide el
//! usuario (texto libre) y devolver un **plan** accionable (qué tool del MCP
//! llamar, qué tarea del tablón crear, con qué Capability).
//!
//! Reglas de este módulo:
//! * **no inventa**: si no hay servicio sano para la petición, lo dice;
//! * **no filtra secretos**: el plan sólo lleva `id`, `kind`, capacidades y
//!   coste; las claves siguen en sus ficheros;
//! * **no ejecuta**: devuelve el plan y el llamador decide (así el MCP y la
//!   web pueden pedir *preview* antes de actuar).

use serde_json::{json, Value};

use crate::service_bus::{ServiceBus, TaskType};

/// Pistas léxicas por tipo de tarea (ES + EN). Orden: gana la primera
/// categoría con más coincidencias; a igualdad, `analysis` (el más conservador).
const HINTS: &[(&str, &[&str])] = &[
    (
        "build",
        &[
            "compil", "build", "implement", "implementa", "arregla", "fix", "refactor",
            "refactoriza", "escribe codigo", "write code", "codigo", "código", "prueba unita",
            "unit test", "patch", "parchea", "commit", "merge", "deploy", "despliega",
            "api", "endpoint", "servidor", "server",
        ],
    ),
    (
        "analysis",
        &[
            "analiza", "analyze", "analisis", "análisis", "decompila", "decompile",
            "revierte", "reverse", "binario", "binary", "dex", "apk", "log", "logs",
            "dependencia", "dependency", "traza", "trace", "busca", "search", "find",
            "reconoce", "fingerprint", "inventario", "inventory", "escanea", "scan",
            "debug", "depurar", "investiga", "investigate", "rootea", "root",
        ],
    ),
    (
        "writing",
        &[
            "redacta", "write", "documenta", "document", "resume", "resumen", "informe",
            "report", "correo", "email", "post", "articulo", "artículo", "presentacion",
            "presentación", "explica", "explain",
        ],
    ),
    (
        "inference",
        &[
            "responde", "answer", "pregunta", "ask", "decide", "decide", "dime", "tell me",
            "opina", "chat", "conversacion", "conversación",
        ],
    ),
];

/// Clasifica una petición en texto libre a un tipo de tarea.
///
/// Determinista y explicable: devuelve también las palabras que dispararon
/// la categoría, para que la decisión sea auditable.
pub fn classify(request: &str) -> (TaskType, Vec<String>) {
    let low = request.to_lowercase();
    let mut best: Option<(&str, TaskType, Vec<String>)> = None;
    for (name, hints) in HINTS {
        let mut hits: Vec<String> = Vec::new();
        for h in *hints {
            if low.contains(h) {
                hits.push((*h).to_string());
            }
        }
        if hits.is_empty() {
            continue;
        }
        let task = match *name {
            "build" => TaskType::Build,
            "analysis" => TaskType::Analysis,
            "writing" => TaskType::Writing,
            _ => TaskType::Inference,
        };
        match &best {
            Some((_, _, prev)) if prev.len() >= hits.len() => {}
            _ => best = Some((name, task, hits)),
        }
    }
    match best {
        Some((_, task, hits)) => (task, hits),
        // Sin pistas: "inference" es el tipo por defecto del bus.
        None => (TaskType::Inference, Vec::new()),
    }
}

/// Herramienta MCP que encaja con el tipo de tarea elegido.
///
/// Mapeo **explícito** (no heurístico): si el bus elige un servicio, la tool
/// es la que ese servicio expone. Para el MCP local (`id == "mcp"`) la tool
/// canónica es `minimax_chat` (el meta-agente local) y para el cliente
/// Antigravity (`id == "antigravity"`) es el propio endpoint MCP.
fn suggested_tool(service_id: &str, task: TaskType) -> String {
    match service_id {
        "antigravity" => "mcp://antigravity/minimax_chat".to_string(),
        "mcp" => match task {
            TaskType::Build => "minimax_chat".to_string(),
            TaskType::Analysis => "dex_decompile".to_string(),
            TaskType::Writing => "minimax_chat".to_string(),
            TaskType::Inference => "minimax_chat".to_string(),
        },
        other => format!("service://{}/invoke", other),
    }
}

/// Construye el **plan** de una petición: qué tarea es, qué servicio la
/// cubre (con su salud real), qué tool llamar y qué tarea del tablón crear.
///
/// No ejecuta nada: el llamador decide. Es lo que permite que Antigravity
/// (vía MCP) y la web (vía HTTP) obtengan **el mismo plan**.
pub fn plan(request: &str, bus: &ServiceBus) -> Value {
    let (task, hits) = classify(request);
    let sel = bus.select(task);
    let chosen = sel.chosen.clone();
    let healthy = chosen
        .as_ref()
        .and_then(|id| bus.entry(id))
        .map(|e| e.health.available)
        .unwrap_or(false);

    let action = match (&chosen, healthy) {
        (Some(id), true) => json!({
            "kind": "dispatch",
            "service": id,
            "tool": suggested_tool(id, task),
            "hint": format!("llama a la tool indicada y devuelve la respuesta real al usuario"),
        }),
        (Some(id), false) => json!({
            "kind": "unhealthy",
            "service": id,
            "tool": suggested_tool(id, task),
            "hint": "el servicio elegido no está sano: el motivo está en `reason`; no inventes la respuesta",
        }),
        (None, _) => json!({
            "kind": "unroutable",
            "hint": "ningún servicio del bus cubre esta petición: repórtalo al tablón como tarea sin dueño",
        }),
    };

    json!({
        "ok": true,
        "request": request,
        "task": task.as_str(),
        "signals": hits,
        "service": chosen,
        "chain": sel.chain,
        "reason": sel.reason,
        "healthy": healthy,
        "estimated_cost_usd": sel.estimated_cost_usd,
        "rejected": sel.rejected.iter().map(|(id, why)| json!({"id": id, "why": why})).collect::<Vec<_>>(),
        "action": action,
        "board_task": {
            "title": format!("[meta] {}", request.chars().take(80).collect::<String>()),
            "task_type": task.as_str(),
            "capabilities": [],
            "owner": "meta_agent",
        },
    })
}

/// Definición de la tool MCP `meta_route`.
pub fn tool_definitions() -> Value {
    json!([{
        "name": "meta_route",
        "description": "Meta-agente: convierte una petición en lenguaje natural en un plan ejecutable (tipo de tarea, servicio del bus con su salud real, tool MCP a la que llamar y tarea del tablón). NO ejecuta: devuelve el plan. Sin secretos.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "request": { "type": "string", "description": "la petición del usuario en texto libre" },
                "execute": { "type": "boolean", "default": false, "description": "si es true, ejecuta el primer paso del plan (tool local) y devuelve la respuesta real" }
            },
            "required": ["request"]
        }
    }])
}

/// Handler de la tool MCP `meta_route`.
pub fn handle(request: &str, execute: bool) -> Value {
    let dir = crate::mesh::default_state_dir();
    let snapshot = match crate::mesh::Mesh::open(&dir) {
        Ok(m) => m.snapshot(),
        Err(e) => json!({ "peers": [], "error": e.to_string() }),
    };
    let bus = crate::service_bus::bus_from_runtime(
        &crate::providers::list_providers_redacted(),
        crate::providers::PROVIDERS_PATH,
        &snapshot,
        300,
        "http://127.0.0.1:9090/mcp",
    );
    let mut plan = plan(request, &bus);
    // Traza: queda en <state_dir>/audit.ndjson (task, servicio, salud, coste).
    // **Sin secretos**: ni la petición ni la respuesta se redactan aquí, pero
    // el cuerpo del model va por el canal del provider, no por este log.
    crate::android::audit(
        "meta_route.plan",
        &json!({
            "request": request.chars().take(200).collect::<String>(),
            "task": plan.get("task").cloned().unwrap_or(Value::Null),
            "service": plan.get("service").cloned().unwrap_or(Value::Null),
            "healthy": plan.get("healthy").cloned().unwrap_or(Value::Null),
            "execute": execute,
        }),
    );
    if !execute {
        plan["executed"] = json!(false);
        return plan;
    }
    // Ejecución mínima: sólo la tool local de inferencia (la única que el
    // meta-agente puede lanzar sin credenciales externas ni coste).
    let tool = plan
        .get("action")
        .and_then(|a| a.get("tool"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    if tool == "minimax_chat" {
        // Misma ruta que la tool `minimax_chat` del MCP (mismo provider, mismo
        // modelo por defecto) => la respuesta es **real**, no un plan.
        let res = crate::providers::execute_chat(request, Some("local-seere"), None, None, 1024);
        plan["executed"] = json!(true);
        plan["result"] = json!({
            "model": res.model_name,
            "latency_ms": res.latency_ms,
            "tokens": res.tokens_used,
            "reply": res.reply,
        });
    } else {
        plan["executed"] = json!(false);
        plan["error"] = json!(format!(
            "ejecución automática no disponible para '{}': el plan indica la tool; ejecútala tú",
            tool
        ));
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service_bus::{ServiceEntry, ServiceKind};

    /// Servicio local sano con TODAS las capacidades del bus (asi lo selecciona
    /// el bus para cualquier tipo de tarea).
    fn bus_con_un_servicio() -> ServiceBus {
        let mut e = ServiceEntry::new("mcp", ServiceKind::McpServer);
        e.health = crate::service_bus::Health::ok(3);
        e.capabilities = vec![
            "inference".to_string(),
            "code".to_string(),
            "analysis".to_string(),
            "writing".to_string(),
        ];
        ServiceBus::new(vec![e])
    }

    #[test]
    fn test_clasifica_peticiones() {
        assert_eq!(classify("compila el crate y arregla el test").0, TaskType::Build);
        assert_eq!(classify("decompila este APK y busca el flag").0, TaskType::Analysis);
        assert_eq!(classify("redacta un informe para el usuario").0, TaskType::Writing);
        assert_eq!(classify("dime hola").0, TaskType::Inference);
        // Sin pistas => inference (el tipo por defecto del bus).
        assert_eq!(classify("").0, TaskType::Inference);
    }

    #[test]
    fn test_plan_devuelve_servicio_y_salud() {
        let bus = bus_con_un_servicio();
        let p = plan("analiza el APK", &bus);
        assert_eq!(p["ok"], json!(true));
        assert_eq!(p["task"], json!("analysis"));
        assert_eq!(p["service"], json!("mcp"));
        assert_eq!(p["healthy"], json!(true));
        assert_eq!(p["action"]["kind"], json!("dispatch"));
        assert_eq!(p["action"]["tool"], json!("dex_decompile"));
        // El plan lleva la señal que lo decidió (auditable).
        assert!(!p["signals"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_plan_sin_servicio_es_honesto() {
        let bus = ServiceBus::new(vec![]);
        let p = plan("analiza el APK", &bus);
        assert_eq!(p["service"], json!(null));
        assert_eq!(p["healthy"], json!(false));
        assert_eq!(p["action"]["kind"], json!("unroutable"));
    }

    #[test]
    fn test_plan_con_servicio_caido_no_dispacha() {
        // Un servicio **comprobado y caido** lo descarta el propio bus
        // (`candidates` lo rechaza con el motivo) => el plan no puede
        //.dispatcharlo y ademas dice POR QUE.
        let mut e = ServiceEntry::new("mcp", ServiceKind::McpServer);
        e.health = crate::service_bus::Health::down("sin tunnel", "530");
        e.capabilities = vec!["analysis".to_string()];
        let bus = ServiceBus::new(vec![e]);
        let p = plan("analiza el APK", &bus);
        assert_eq!(p["service"], json!(null));
        assert_eq!(p["healthy"], json!(false));
        assert_eq!(p["action"]["kind"], json!("unroutable"));
        let rejected = p["rejected"].as_array().unwrap();
        assert!(rejected
            .iter()
            .any(|r| r["why"].as_str().unwrap_or("").contains("caído")));
    }

    #[test]
    fn test_tool_definitions_declara_meta_route() {
        let defs = tool_definitions();
        let arr = defs.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], json!("meta_route"));
        assert!(arr[0]["inputSchema"]["properties"]["request"].is_object());
    }
}
