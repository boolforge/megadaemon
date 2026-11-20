//! MegaDaemon v15 (W6) — **Bus de servicios**: humo del catálogo unificado,
//! de la selección con fallback y del presupuesto.
//!
//! Estos tests NO tocan la red ni el disco de estado: trabajan sobre catálogos
//! construidos a mano y sobre los adaptadores puros.
//!
//! Ejecutar:
//!   cd rust-core && cargo test --test service_bus_smoke

use megadaemon_core::service_bus::{
    antigravity_bridge_config, bridge_with_key, bus_from_runtime, catalog_from_providers_json,
    entry_from_json, merge_bridge_config, seed_antigravity_client, Budget, CostEstimate, Health,
    Prober, SelectionPolicy, ServiceBus, ServiceEntry, ServiceKind, TaskType, TcpProber,
    UnknownProber,
};
use serde_json::{json, Value};

fn model(id: &str, caps: &[&str], free: bool) -> ServiceEntry {
    let mut e = ServiceEntry::new(id, ServiceKind::Model);
    e.capabilities = caps.iter().map(|c| c.to_string()).collect();
    e.cost_estimate = if free {
        CostEstimate::free()
    } else {
        CostEstimate::per_token(0.002, 0.004)
    };
    e
}

#[test]
fn el_catalogo_tiene_los_campos_del_plan_maestro() {
    let mut e = model("minimax-m3.1-flash", &["inference"], false);
    e.endpoint = "https://api.minimax.io/v1/chat/completions".into();
    e.auth_kind = "bearer".into();
    e.auth_ref = "/state/providers.json".into();
    let bus = ServiceBus::new(vec![e]);
    let j = bus.catalog_json();
    let entry = &j["entries"][0];
    for f in [
        "id",
        "kind",
        "endpoint",
        "auth_kind",
        "health",
        "latency_ms",
        "cost_estimate",
        "capabilities",
        "enabled",
    ] {
        assert!(entry.get(f).is_some(), "falta el campo {}", f);
    }
    assert_eq!(entry["kind"], json!("model"));
}

#[test]
fn la_salud_nunca_se_inventa() {
    // Una entrada recién hecha no está "disponible": está "sin comprobar".
    let bus = ServiceBus::new(vec![model("a", &["inference"], true)]);
    let j = bus.catalog_json();
    assert_eq!(j["entries"][0]["health"]["available"], json!(false));
    assert_eq!(j["entries"][0]["health"]["reason"], json!("sin comprobar"));
    assert!(j["entries"][0]["health"]["checked_at"].is_null());
    // Y la sonda "desconocida" tampoco afirma nada.
    let h = UnknownProber.probe(&ServiceEntry::new("x", ServiceKind::Model));
    assert!(!h.available);
    assert!(h.checked_at.is_none());
}

#[test]
fn la_seleccion_es_parasita_y_tiene_fallback() {
    let mut caro = model("caro", &["inference"], false);
    caro.cost_estimate = CostEstimate::per_token(0.01, 0.02);
    let mut gratis = model("gratis", &["inference"], true);
    gratis.health = Health::ok(12);
    let bus = ServiceBus::new(vec![caro.clone(), gratis.clone()]);
    let sel = bus.select(TaskType::Inference);
    assert_eq!(sel.chosen.as_deref(), Some("gratis"));
    assert_eq!(sel.chain.len(), 2);

    // Con el gratis caído (comprobado), entra el de pago como último recurso.
    let mut bus2 = ServiceBus::new(vec![caro.clone(), gratis.clone()]);
    bus2.set_health("gratis", Health::down("caído", "500")).unwrap();
    let sel2 = bus2.select(TaskType::Inference);
    assert_eq!(sel2.chosen.as_deref(), Some("caro"));

    // Y el fallback real: si el primero falla, responde el segundo.
    let mut bus3 = ServiceBus::new(vec![caro, gratis]);
    let mut calls = Vec::new();
    let out = bus3.route(TaskType::Inference, &mut |e| {
        calls.push(e.id.clone());
        if e.id == "caro" {
            Err("timeout".into())
        } else {
            Ok("ok".into())
        }
    });
    assert!(out.ok);
    assert_eq!(out.used.as_deref(), Some("gratis"));
    assert_eq!(calls.len(), 2);
}

#[test]
fn el_presupuesto_manda() {
    let mut caro = model("caro", &["inference"], false);
    caro.cost_estimate = CostEstimate::per_token(5.0, 10.0);
    let mut bus = ServiceBus::new(vec![caro]);
    bus.set_budget(Budget::new(0.001, 1.0));
    let sel = bus.select(TaskType::Inference);
    assert!(sel.chosen.is_none());
    assert!(sel
        .rejected
        .iter()
        .any(|(_, why)| why.contains("presupuesto")));
}

#[test]
fn el_catalogo_no_lleva_secretos() {
    let mut e = model("minimax", &["inference"], false);
    e.auth_kind = "bearer".into();
    e.auth_ref = "/state/providers.json".into();
    e.notes = "la credencial vive en el fichero".into();
    let mut ag = seed_antigravity_client("http://127.0.0.1:9090/mcp");
    ag.auth_ref = "md-secrets/instance.env".into();
    let bus = ServiceBus::new(vec![e, ag]);
    let j = bus.catalog_json().to_string();
    for bad in ["ghp_", "gho_", "Bearer ", "mdaemon_sk_", "api_key\":"] {
        assert!(!j.contains(bad), "el catálogo filtró {:?}", bad);
    }
    // Pero sí dice cómo y dónde.
    assert!(j.contains("\"auth_kind\":\"bearer\""));
    assert!(j.contains("/state/providers.json"));
}

#[test]
fn el_catalogo_real_se_construye_de_providers_y_mesh() {
    let providers = json!({
        "providers": [
            {"id":"minimax-m3.1-flash","name":"MiniMax","endpoint":"https://api.minimax.io/v1/chat/completions",
             "model":"MiniMax-M3.1-Flash","auth_type":"bearer","tier":"paid","enabled":true,"ready":true},
            {"id":"local-seere","name":"SEERE","endpoint":"http://127.0.0.1:8080/v1/chat/completions",
             "model":"local","auth_type":"local","tier":"offline","enabled":true,"ready":true}
        ]
    });
    let mesh = json!({
        "peers": [
            {"id":"cloud-sandbox","hostname":"cloud","capabilities":["gpu"],"last_seen": 4102444800i64},
            {"id":"muerto","hostname":"x","capabilities":[],"last_seen": 0}
        ]
    });
    let bus = bus_from_runtime(
        &providers,
        "/state/providers.json",
        &mesh,
        300,
        "http://127.0.0.1:9090/mcp",
    );
    let j = bus.catalog_json();
    assert_eq!(j["counts"]["models"], json!(2));
    assert_eq!(j["counts"]["agents"], json!(2));
    // El peer muerto NO está disponible (y se dice por qué).
    let ids: Vec<&str> = j["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["id"].as_str())
        .collect();
    assert!(ids.contains(&"muerto"));
    let muerto = j["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == json!("muerto"))
        .unwrap();
    assert_eq!(muerto["health"]["available"], json!(false));
    assert!(muerto["health"]["reason"].as_str().unwrap().contains("contacto"));
    // El cliente MCP de Antigravity está catalogado.
    assert!(ids.contains(&"antigravity"));
}

#[test]
fn la_voz_elige_el_cloud_y_cae_al_local() {
    let mut cloud = model("minimax", &["inference"], false);
    cloud.endpoint = "https://api.minimax.io/v1/chat/completions".into();
    cloud.health = Health::ok(200);
    let mut local = model("local-seere", &["inference"], true);
    local.endpoint = "http://127.0.0.1:8080/v1/chat/completions".into();
    local.health = Health::ok(5);
    let bus = ServiceBus::new(vec![local.clone(), cloud.clone()]);
    assert_eq!(bus.voice_model().as_deref(), Some("minimax"));
    // Sin red (cloud caído): el SEERE local.
    let mut bus2 = ServiceBus::new(vec![local, cloud]);
    bus2.set_health("minimax", Health::down("caído", "sin red")).unwrap();
    assert_eq!(bus2.voice_model().as_deref(), Some("local-seere"));
    // Y la política "capaz" pone el local el último.
    let sel = bus2.select_with(SelectionPolicy::Best, TaskType::Inference);
    assert_eq!(sel.chain.last().map(|s| s.as_str()), Some("local-seere"));
}

#[test]
fn el_puente_de_antigravity_no_lleva_la_clave() {
    let cfg = antigravity_bridge_config(
        "http://127.0.0.1:9090/mcp",
        "md-secrets/instance.env",
        &["minimax_chat", "swarm_chat"],
    );
    let j = cfg.to_string();
    assert!(j.contains("X-API-Key"));
    assert!(j.contains("X-Seere-Auth-Token"));
    assert!(j.contains("minimax_chat"));
    assert!(!j.contains("mdaemon_sk_"));
    // El nombre del servidor es el que espera Antigravity.
    assert!(j.contains("megadaemon-minimax"));
    let con = bridge_with_key(cfg, "mdaemon_sk_REAL");
    assert!(con.to_string().contains("mdaemon_sk_REAL"));
}

#[test]
fn el_merge_del_puente_conserva_providers_y_las_cabeceras_reales() {
    let existing = r#"{
        "mcpServers": {
            "megadaemon-minimax": {
                "url": "http://127.0.0.1:9090/mcp",
                "headers": {"X-API-Key": "REAL", "X-Seere-Auth-Token": "TOK"}
            }
        },
        "providers": {"minimax": {"api_key": "sk-cp-secreto"}}
    }"#;
    let bridge = antigravity_bridge_config(
        "http://127.0.0.1:9090/mcp",
        "md-secrets/instance.env",
        &[],
    );
    let merged = merge_bridge_config(existing, &bridge).unwrap();
    let v: Value = serde_json::from_str(&merged).unwrap();
    assert_eq!(v["providers"]["minimax"]["api_key"], json!("sk-cp-secreto"));
    assert_eq!(
        v["mcpServers"]["megadaemon-minimax"]["headers"]["X-API-Key"],
        json!("REAL")
    );
    assert_eq!(
        v["mcpServers"]["megadaemon-minimax"]["headers"]["X-Seere-Auth-Token"],
        json!("TOK")
    );
    assert!(!merged.contains("${"));
}

#[test]
fn el_adaptador_ingiere_la_entrada_de_w5() {
    let v = json!({
        "id": "antigravity", "kind": "agent_client", "endpoint": "http://127.0.0.1:9090/mcp",
        "auth_kind": "session_token_file", "health": "stdio roto, http vivo", "healthy": true,
        "tools_count": 7, "capabilities": ["mcp_client"], "cost_estimate": "included", "enabled": true
    });
    let e = entry_from_json(&v).expect("entrada válida");
    assert_eq!(e.kind, ServiceKind::AgentClient);
    assert!(e.health.available);
    assert!(e.cost_estimate.free);
    // Sin "healthy" no se inventa disponibilidad.
    let e2 = entry_from_json(&json!({"id": "x", "kind": "agent"})).unwrap();
    assert!(!e2.health.available);
    assert_eq!(e2.health.reason, "sin comprobar");
}

#[test]
fn la_sonda_tcp_no_confunde_un_puerto_cerrado_con_salud() {
    let prober = TcpProber { timeout_ms: 300 };
    let mut e = ServiceEntry::new("x", ServiceKind::Model);
    e.endpoint = "http://127.0.0.1:1/nada".into();
    let h = prober.probe(&e);
    assert!(!h.available, "un puerto cerrado no es salud");
    assert!(h.checked_at.is_some(), "pero sí se comprobó");
    let _: Value = json!({});
}
