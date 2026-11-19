//! MegaDaemon v13.4 — Núcleo agéntico.
//!
//! Convierte un **objetivo en lenguaje natural** en una lista de tareas
//! ejecutables, las encola en el planificador (`crate::task_scheduler`) y
//! hace el seguimiento.  Además es el punto donde se decide **qué LLM usar**:
//!
//! * **oportunista**: primero el LLM local (SEERE) —0 tokens, sin red—;
//! * si el local no está o no puede, se cae al enjambre remoto (`swarm`);
//! * si no hay ningún LLM disponible, se usa un **planificador heurístico**
//!   determinista que funciona sin modelo.
//!
//! El planificador heurístico no es un adorno: garantiza que el agente
//! **siempre** puede actuar, incluso en un móvil sin red y sin modelo local.
//!
//! El ejecutor de tareas (`execute`) es el punto de extensión donde se
//! conectan las capacidades: cada `kind` de tarea se resuelve con los módulos
//! reales (sync, apk, updates, alerts, messaging...).  Un `kind` desconocido
//! se rechaza con error explícito: nunca se ejecuta shell libre.

use serde_json::{json, Value};
use std::collections::HashMap;

/// Objetivo → plan de tareas.
#[derive(Clone, Debug)]
pub struct Plan {
    pub goal: String,
    pub tasks: Vec<PlannedTask>,
    pub source: String,
    pub notes: String,
}

/// Tarea planificada (aún no encolada).
#[derive(Clone, Debug)]
pub struct PlannedTask {
    pub kind: String,
    pub payload: Value,
    pub priority: u8,
    pub deps: Vec<String>,
}

/// Clasificador heurístico: palabras clave → tipo de tarea.
///
/// Es determinista y no depende de ningún modelo.
fn heuristic_plan(goal: &str) -> Plan {
    let g = goal.to_lowercase();
    let mut tasks: Vec<PlannedTask> = vec![];
    let mut notes = String::new();

    let has = |k: &[&str]| k.iter().any(|w| g.contains(w));

    if has(&["sincroniz", "sync", "conversacion", "chatgpt", "claude", "deepseek", "kimi", "historial"]) {
        tasks.push(PlannedTask {
            kind: "sync.local".into(),
            payload: json!({}),
            priority: 7,
            deps: vec![],
        });
        tasks.push(PlannedTask {
            kind: "sync.web".into(),
            payload: json!({}),
            priority: 5,
            deps: vec![],
        });
    }
    if has(&["buscar", "search", "encuentra", "recupera", "localiza"]) {
        tasks.push(PlannedTask {
            kind: "search.conversations".into(),
            payload: json!({ "query": goal }),
            priority: 6,
            deps: vec![],
        });
    }
    if has(&["apk", "aplicacion", "app", "paquete", "instal", "actualiz", "update", "repo"]) {
        tasks.push(PlannedTask {
            kind: "apk.inventory".into(),
            payload: json!({}),
            priority: 5,
            deps: vec![],
        });
        tasks.push(PlannedTask {
            kind: "updates.plan".into(),
            payload: json!({}),
            priority: 4,
            deps: vec![],
        });
    }
    if has(&["perfil", "interes", "fortaleza", "debilidad", "psicolog", "adiccion", "pendiente"]) {
        tasks.push(PlannedTask {
            kind: "profile.analyze".into(),
            payload: json!({}),
            priority: 6,
            deps: vec![],
        });
    }
    if has(&["telegram", "whatsapp", "sms", "mensaje", "cita", "medic", "salud"]) {
        tasks.push(PlannedTask {
            kind: "messaging.ingest".into(),
            payload: json!({}),
            priority: 6,
            deps: vec![],
        });
    }
    if has(&["banco", "bbva", "a eat", "aeat", "fiscal", "root", "stealth", "ocult"]) {
        tasks.push(PlannedTask {
            kind: "root.stealth.status".into(),
            payload: json!({}),
            priority: 5,
            deps: vec![],
        });
    }
    if has(&["alerta", "aviso", "vigila", "monitor", "controla"]) {
        tasks.push(PlannedTask {
            kind: "alerts.check".into(),
            payload: json!({}),
            priority: 8,
            deps: vec![],
        });
    }
    if has(&["dispositivo", "movil", "telefono", "bateria", "memoria", "almacenamiento", "espacio"]) {
        tasks.push(PlannedTask {
            kind: "device.overview".into(),
            payload: json!({}),
            priority: 6,
            deps: vec![],
        });
    }
    if has(&["web", "scrap", "pagina", "sitio"]) {
        tasks.push(PlannedTask {
            kind: "web.scrape".into(),
            payload: json!({ "url": extract_url(goal).unwrap_or_default() }),
            priority: 4,
            deps: vec![],
        });
    }

    if tasks.is_empty() {
        notes.push_str("Sin coincidencia heurística: se ejecuta el resumen del dispositivo y el estado del sistema.");
        tasks.push(PlannedTask {
            kind: "device.overview".into(),
            payload: json!({}),
            priority: 5,
            deps: vec![],
        });
        tasks.push(PlannedTask {
            kind: "capabilities.verify".into(),
            payload: json!({}),
            priority: 4,
            deps: vec![],
        });
    }
    Plan {
        goal: goal.to_string(),
        tasks,
        source: "heuristic".into(),
        notes,
    }
}

/// Extrae la primera URL de un texto.
fn extract_url(text: &str) -> Option<String> {
    let idx = text.find("http")?;
    let rest = &text[idx..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
        .unwrap_or(rest.len());
    Some(rest[..end].trim_end_matches(['.', ',', ';']).to_string())
}

/// Intenta planificar con el LLM local (SEERE). Si no puede, `None`.
fn llm_plan(goal: &str) -> Option<Plan> {
    let system = "Eres el planificador de MegaDaemon. Responde SOLO con líneas \
                  'kind|payload_json' de tareas ejecutables. Kinds válidos: \
                  sync.local, sync.web, search.conversations, apk.inventory, \
                  updates.plan, profile.analyze, messaging.ingest, alerts.check, \
                  device.overview, web.scrape, capabilities.verify. Máximo 4 líneas.";
    let res = crate::providers::execute_chat(goal, Some("local-seere"), None, Some(system), 512);
    if res.reply.trim().is_empty() {
        return None;
    }
    let mut tasks = vec![];
    for line in res.reply.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (kind, payload) = match line.split_once('|') {
            Some((k, p)) => (k.trim().to_string(), p.trim().to_string()),
            None => (line.to_string(), "{}".to_string()),
        };
        if !is_known_kind(&kind) {
            continue;
        }
        let payload: Value = serde_json::from_str(&payload).unwrap_or_else(|_| json!({ "query": goal }));
        tasks.push(PlannedTask {
            kind,
            payload,
            priority: 5,
            deps: vec![],
        });
        if tasks.len() >= 4 {
            break;
        }
    }
    if tasks.is_empty() {
        None
    } else {
        Some(Plan {
            goal: goal.to_string(),
            tasks,
            source: "local-seere".into(),
            notes: "Plan generado por el LLM local (0 tokens).".into(),
        })
    }
}

/// Kinds conocidos (lista blanca: nunca se ejecuta lo que no esté aquí).
pub fn is_known_kind(kind: &str) -> bool {
    const KINDS: &[&str] = &[
        "sync.local",
        "sync.web",
        "sync.harvest",
        "search.conversations",
        "apk.inventory",
        "apk.analyze",
        "updates.plan",
        "updates.status",
        "profile.analyze",
        "profile.briefing",
        "messaging.ingest",
        "messaging.inventory",
        "alerts.check",
        "alerts.list",
        "device.overview",
        "device.apps",
        "web.scrape",
        "capabilities.verify",
        "root.stealth.status",
        "root.stealth.apply",
        "tasks.list",
    ];
    KINDS.contains(&kind)
}

/// Planifica un objetivo. `prefer_llm` intenta el LLM local primero.
pub fn plan(goal: &str, prefer_llm: bool) -> Plan {
    if prefer_llm {
        if let Some(p) = llm_plan(goal) {
            return p;
        }
    }
    heuristic_plan(goal)
}

/// Ejecuta un objetivo completo: plan → encolar → ejecutar → resumen.
pub fn run(goal: &str, prefer_llm: bool, max_tasks: usize) -> Value {
    let p = plan(goal, prefer_llm);
    let mut ids = vec![];
    for t in &p.tasks {
        let res = crate::task_scheduler::Resources {
            cpu: if t.kind.starts_with("sync") || t.kind == "messaging.ingest" { 2 } else { 1 },
            net: matches!(t.kind.as_str(), "sync.web" | "updates.plan" | "web.scrape" | "messaging.ingest"),
            heavy: matches!(t.kind.as_str(), "messaging.ingest"),
        };
        let id = crate::task_scheduler::submit(&t.kind, t.payload.clone(), t.priority, t.deps.clone(), res);
        ids.push(json!({ "id": id, "kind": t.kind }));
    }
    let results = crate::task_scheduler::run_due(max_tasks.max(1));
    let ok_count = results
        .iter()
        .filter(|r| r["ok"].as_bool().unwrap_or(false))
        .count();
    json!({
        "ok": true,
        "goal": goal,
        "plan_source": p.source,
        "notes": p.notes,
        "tasks": ids,
        "executed": results.len(),
        "succeeded": ok_count,
        "results": results,
    })
}

/// **Ejecutor de tareas**: resuelve cada `kind` con los módulos reales.
pub fn execute(kind: &str, payload: &Value) -> Value {
    match kind {
        "sync.local" => {
            let services = payload.get("services").and_then(|v| v.as_array()).map(|a| {
                a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect::<Vec<String>>()
            });
            let r = crate::sync::run_sync(services, false);
            json!({ "ok": r["ok"].clone(), "summary": format!(
                "Sincronizadas {} conversaciones ({} nuevas, {} actualizadas)",
                r["total_conversations"], r["added"], r["updated"]
            ), "detail": r })
        }
        "sync.web" => {
            let r = crate::sync_web::web_pull(None);
            json!({ "ok": true, "summary": format!("Pull web: {} conversaciones", r["pulled"]), "detail": r })
        }
        "sync.harvest" => {
            let r = crate::sync_web::harvest_sessions();
            json!({ "ok": true, "summary": format!("Sesiones cosechadas: {}", r["sessions"]), "detail": r })
        }
        "search.conversations" => {
            let q = payload.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let svc = payload.get("service").and_then(|v| v.as_str());
            let limit = payload.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let r = crate::sync::search(q, svc, limit);
            json!({ "ok": true, "summary": format!("{} resultados para '{}'", r["total_hits"], q), "detail": r })
        }
        "apk.inventory" => {
            let limit = payload.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let r = crate::apk_inventory::inventory(limit);
            json!({ "ok": r["ok"].clone(), "summary": format!("{} APKs inventariadas", r["count"].clone()), "detail": r })
        }
        "apk.analyze" => {
            let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let r = crate::apk_inventory::analyze_apk(path);
            json!({ "ok": r["ok"].clone(), "summary": format!("APK analizada: {}", path), "detail": r })
        }
        "updates.plan" => {
            let r = crate::repo_updates::plan(None);
            json!({ "ok": true, "summary": format!("{} actualizaciones disponibles", r["updatable"].clone()), "detail": r })
        }
        "updates.status" => {
            let r = crate::repo_updates::status();
            json!({ "ok": true, "summary": "Estado de repositorios", "detail": r })
        }
        "profile.analyze" => {
            let r = crate::profile_analysis::analyze();
            json!({ "ok": true, "summary": "Perfil generado", "detail": r })
        }
        "profile.briefing" => {
            let md = payload.get("markdown").and_then(|v| v.as_bool()).unwrap_or(true);
            let r = crate::profile_analysis::briefing(md);
            json!({ "ok": true, "summary": "Informe de perfil generado", "detail": r })
        }
        "messaging.ingest" => {
            let r = crate::messaging::ingest_all();
            json!({ "ok": true, "summary": "Mensajería ingestada", "detail": r })
        }
        "messaging.inventory" => {
            let r = crate::messaging::inventory();
            json!({ "ok": true, "summary": "Inventario de mensajería", "detail": r })
        }
        "alerts.check" => {
            let r = crate::alerts::check_rules();
            json!({ "ok": true, "summary": format!("{} alertas generadas", r["raised"].clone()), "detail": r })
        }
        "alerts.list" => {
            let r = crate::alerts::list(None, false, 50);
            json!({ "ok": true, "summary": format!("{} alertas activas", r["total"].clone()), "detail": r })
        }
        "device.overview" => {
            let r = crate::android::system_overview();
            json!({ "ok": true, "summary": "Resumen del dispositivo", "detail": r })
        }
        "device.apps" => {
            let scope = payload.get("scope").and_then(|v| v.as_str()).unwrap_or("third_party");
            let r = crate::android::app_control("list", scope, "", "", "", "", "", false);
            json!({ "ok": true, "summary": format!("{} apps", r["count"].clone()), "detail": r })
        }
        "web.scrape" => {
            let url = payload.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if url.is_empty() {
                return json!({ "ok": false, "error": "missing_url" });
            }
            let ident = payload.get("identity").and_then(|v| v.as_str()).unwrap_or("browser.mobile");
            let respect = payload.get("respect_robots").and_then(|v| v.as_bool()).unwrap_or(true);
            let r = crate::service_identity::scrape(url, ident, respect, 512 * 1024);
            json!({ "ok": r["ok"].clone(), "summary": format!("Scrape {} → {} bytes", url, r["bytes"].clone()), "detail": r })
        }
        "capabilities.verify" => {
            let r = crate::capability::verify();
            json!({ "ok": true, "summary": format!("{}/{} capacidades sanas", r["healthy"].clone(), r["total"].clone()), "detail": r })
        }
        "root.stealth.status" => {
            let r = crate::root_stealth::status();
            json!({ "ok": true, "summary": "Estado del perfil de ocultación", "detail": r })
        }
        "root.stealth.apply" => {
            let confirm = payload.get("confirm").and_then(|v| v.as_bool()).unwrap_or(false);
            let scope = payload.get("scope").and_then(|v| v.as_str()).unwrap_or("all");
            let r = crate::root_stealth::apply(scope, confirm);
            json!({ "ok": r["ok"].clone(), "summary": "Perfil de ocultación aplicado", "detail": r })
        }
        "tasks.list" => {
            let r = crate::task_scheduler::list_json();
            json!({ "ok": true, "summary": format!("{} tareas", r["tasks"].as_array().map(|a| a.len()).unwrap_or(0)), "detail": r })
        }
        other => json!({
            "ok": false,
            "error": "unknown_kind",
            "hint": format!("tipo de tarea no soportado: {}", other),
        }),
    }
}

/// Estado del agente y de sus decisiones de enrutamiento.
pub fn status() -> Value {
    let caps = crate::capability::verify();
    let queue = crate::task_scheduler::stats();
    let mut kinds: HashMap<String, usize> = HashMap::new();
    for k in is_known_kind_list() {
        kinds.insert(k.to_string(), 0);
    }
    json!({
        "ok": true,
        "known_task_kinds": is_known_kind_list(),
        "capabilities": { "total": caps["total"].clone(), "healthy": caps["healthy"].clone() },
        "queue": queue,
        "llm_route": {
            "primary": "local-seere",
            "policy": "oportunista: local primero (0 tokens), swarm remoto si el local falla, heurística determinista si no hay LLM",
        },
        "kind_count": kinds.len(),
    })
}

/// Lista de tipos de tarea soportados.
pub fn is_known_kind_list() -> Vec<&'static str> {
    vec![
        "sync.local", "sync.web", "sync.harvest", "search.conversations", "apk.inventory",
        "apk.analyze", "updates.plan", "updates.status", "profile.analyze", "profile.briefing",
        "messaging.ingest", "messaging.inventory", "alerts.check", "alerts.list", "device.overview",
        "device.apps", "web.scrape", "capabilities.verify", "root.stealth.status",
        "root.stealth.apply", "tasks.list",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_planificador_heuristico_reconoce_las_intenciones() {
        let p = heuristic_plan("sincroniza mis conversaciones de chatgpt y deepseek");
        assert!(p.tasks.iter().any(|t| t.kind == "sync.local"));
        assert!(p.tasks.iter().any(|t| t.kind == "sync.web"));

        let p2 = heuristic_plan("haz un inventario de todas las apks instaladas");
        assert!(p2.tasks.iter().any(|t| t.kind == "apk.inventory"));

        let p3 = heuristic_plan("analiza mi perfil para el psicólogo");
        assert!(p3.tasks.iter().any(|t| t.kind == "profile.analyze"));

        let p4 = heuristic_plan("hazme un resumen de lo que no sé");
        assert!(!p4.tasks.is_empty());
    }

    #[test]
    fn un_objetivo_desconocido_siempre_produce_un_plan() {
        let p = heuristic_plan("xyzzyloquid");
        assert!(!p.tasks.is_empty());
        assert_eq!(p.source, "heuristic");
    }

    #[test]
    fn solo_se_ejecutan_kinds_conocidos() {
        assert!(is_known_kind("sync.local"));
        assert!(!is_known_kind("rm -rf /"));
        assert!(!is_known_kind("exec"));
    }

    #[test]
    fn ejecutar_un_kind_desconocido_falla_sin_hacer_nada() {
        let r = execute("rm -rf /", &json!({}));
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("unknown_kind"));
    }

    #[test]
    fn extrae_urls() {
        assert_eq!(
            extract_url("mira https://x.test/pagina y avisa").as_deref(),
            Some("https://x.test/pagina")
        );
        assert_eq!(extract_url("sin url"), None);
    }

    #[test]
    fn el_estado_es_consistente() {
        let s = status();
        assert_eq!(s["ok"], json!(true));
        assert!(s["known_task_kinds"].as_array().unwrap().len() >= 15);
    }
}
