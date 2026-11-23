//! Arnés de la **asimilación de Antigravity**.
//!
//! Qué se prueba (todo **puro**, sin red y sin móvil):
//!
//!   1. El **dialecto Gemini** del gateway canónico: es el idioma que habla
//!      el language server de Antigravity (MEDIDO 2026-11-22 en
//!      `liblanguage_server.so`: `genai.Models.generateContent` y las rutas
//!      `/v1internal:generateContent` / `:streamGenerateContent`).
//!   2. El **BYOM**: el `ModelInfo` que registra Antigravity tiene que
//!      tener **exactamente** los mismos campos que construye su GUI
//!      (`modelId`, `apiProvider`, `chatModelName`, `modelName`, `maxTokens`,
//!      `modelFeatures`), porque el que lee y escribe es el mismo proto.
//!   3. La **credencial de proveedor** se escribe en `mcp_config.json` sin
//!      romper lo que ya había.
//!   4. El **entorno absorbido** de Antigravity (el userland Termux) se
//!      cataloga sin petar cuando no hay móvil.
//!   5. La **jerarquía**: `antigravity_clone` ya **no** tiene motor de pull
//!      propio (el segundo motor): delega en el mecanismo canónico.
//!   6. La **paridad**: el catálogo MCP anuncia las tools nuevas y las
//!      despacha.

use megadaemon_core::antigravity_clone;
use megadaemon_core::antigravity_env as ae;
use megadaemon_core::antigravity_model as am;
use megadaemon_core::llm_gateway as gw;
use serde_json::json;

// ---------------------------------------------------------------------------
// 1. Dialecto Gemini (el idioma de Antigravity)
// ---------------------------------------------------------------------------

#[test]
fn las_rutas_del_language_server_se_reconocen() {
    // MEDIDO 2026-11-22: strings de `liblanguage_server.so`.
    let r = gw::gemini_route("/v1internal:generateContent").expect("ruta v1internal");
    assert!(r.model.is_empty());
    assert!(!r.stream);

    let r = gw::gemini_route("/v1internal:streamGenerateContent").expect("ruta stream");
    assert!(r.stream);

    let r = gw::gemini_route("/v1beta/models/megadaemon:generateContent").expect("ruta v1beta");
    assert_eq!(r.model, "megadaemon");

    assert!(gw::gemini_route("/v1/chat/completions").is_none());
    assert!(gw::gemini_route("/api/status").is_none());
    assert!(gw::gemini_route("/").is_none());
}

#[test]
fn la_peticion_gemini_se_traduce_a_prompt() {
    let body = json!({
        "contents": [
            {"role": "user", "parts": [{"text": "resume esto"}]}
        ],
        "systemInstruction": {"parts": [{"text": "eres conciso"}]},
        "generationConfig": {"maxOutputTokens": 512, "temperature": 0.2}
    });
    let g = gw::gemini_prompt(&body).expect("petición válida");
    assert_eq!(g.prompt, "resume esto");
    assert_eq!(g.system.as_deref(), Some("eres conciso"));
    assert_eq!(g.max_tokens, 512);
}

#[test]
fn una_peticion_gemini_vacia_da_error_claro() {
    let e = gw::gemini_prompt(&json!({"contents": []})).expect_err("debe fallar");
    // El error dice QUE falta: no un 500 opaco.
    assert!(!e.message.is_empty());
    assert!(e.message.contains("texto") || e.message.contains("contents"), "mensaje: {}", e.message);
}

// ---------------------------------------------------------------------------
// 2. El BYOM: el modelo de Antigravity es el gateway de MegaDaemon
// ---------------------------------------------------------------------------

#[test]
fn el_modelo_personalizado_apunta_al_gateway_canonico() {
    let spec = am::ModelSpec::default();
    assert_eq!(spec.label, "megadaemon");
    assert_eq!(spec.base_url, "http://127.0.0.1:9090");

    let info = am::model_info(&spec);
    // Mismos nombres de campo que la GUI (main.js).
    for k in [
        "modelId",
        "apiProvider",
        "chatModelName",
        "modelName",
        "maxTokens",
        "modelFeatures",
    ] {
        assert!(info.get(k).is_some(), "falta el campo {k}");
    }
    assert_eq!(info["modelId"], "GOOGLE_GEMINI_INTERNAL_BYOM");
    assert_eq!(info["apiProvider"], "API_PROVIDER_GOOGLE_EVERGREEN");
    assert_eq!(info["modelName"], "http://127.0.0.1:9090");
    // El modelo tiene que poder usar tools: es lo que hace que Antigravity
    // pueda llamar a las tools de MegaDaemon.
    assert_eq!(info["modelFeatures"]["supportsToolCalls"], true);
}

#[test]
fn el_payload_de_estado_es_el_que_entiende_la_ide() {
    let modelos = vec![(
        "megadaemon".to_string(),
        am::model_info(&am::ModelSpec::default()),
    )];
    let p = am::app_state_payload(&modelos);
    // appState.customModelsConfig.customModels.<etiqueta>
    let m = &p["appState"]["customModelsConfig"]["customModels"]["megadaemon"];
    assert_eq!(m["chatModelName"], "megadaemon");

    // Y se relee igual.
    let leidos = am::custom_models_from_app_state(&p["appState"]);
    assert_eq!(leidos.len(), 1);
    assert_eq!(leidos[0].0, "megadaemon");
}

#[test]
fn el_resumen_del_modelo_no_trae_secretos() {
    let s = am::summary(&am::model_info(&am::ModelSpec::default()));
    let texto = s.to_string();
    assert!(!texto.contains("api_key"));
    assert!(!texto.contains("apiKey"));
    assert_eq!(s["label"], "megadaemon");
}

// ---------------------------------------------------------------------------
// 3. La credencial del proveedor
// ---------------------------------------------------------------------------

#[test]
fn la_credencial_se_escribe_sin_romper_el_resto() {
    let original = json!({
        "mcpServers": {
            "megadaemon-minimax": {"url": "http://127.0.0.1:9090/mcp", "headers": {"X-API-Key": "K"}}
        },
        "providers": {"minimax": {"api_key": "VIEJA"}}
    })
    .to_string();

    let nuevo = am::patch_provider_key(&original, "megadaemon", "NUEVA").expect("parche ok");
    assert_eq!(am::provider_key(&nuevo, "megadaemon").as_deref(), Some("NUEVA"));
    assert_eq!(am::provider_key(&nuevo, "minimax").as_deref(), Some("VIEJA"));
    // El servidor MCP sigue intacto.
    assert!(nuevo.contains("127.0.0.1:9090/mcp"));
}

// ---------------------------------------------------------------------------
// 4. El entorno absorbido de Antigravity
// ---------------------------------------------------------------------------

#[test]
fn el_entorno_se_cataloga_sin_petar_sin_movil() {
    let r = ae::environment_report(1_000);
    assert_eq!(r["ok"], true);
    assert!(r["runtime_bin"].as_str().unwrap().contains("runtime/bin"));
    assert!(r["herramientas_clave"].is_array());
}

#[test]
fn las_herramientas_clave_son_las_que_tenia_antigravity() {
    // MEDIDO 2026-11-22 en el móvil: git 2.56.0, python3 3.14.10,
    // openjdk 17.0.21.1, aapt 1.360.584 B, zipalign, llvm-ar, node, curl.
    for t in ["git", "python3", "java", "apksigner", "aapt", "zipalign", "llvm-ar", "curl"] {
        assert!(
            ae::KEY_TOOLS.contains(&t),
            "falta la herramienta clave {t}"
        );
    }
}

#[test]
fn resolver_una_herramienta_inexistente_no_peta() {
    assert!(ae::resolve("no-existe-jamas-esta").is_none());
    let r = ae::tool_report("no-existe-jamas-esta", 1_000);
    assert_eq!(r["ok"], false);
}

// ---------------------------------------------------------------------------
// 5. Jerarquía: el clon ya no tiene motor de pull propio
// ---------------------------------------------------------------------------

#[test]
fn el_clon_delega_el_pull_en_el_mecanismo_canonico() {
    // En el sandbox no hay móvil: no hay instalador canónico ⇒ el informe
    // dice que el clon NO tiene motor propio (y no intenta bajarse nada).
    let r = antigravity_clone::sincronizar_codigo().expect("informe");
    assert!(!r.aplicado);
    assert_eq!(r.mecanismo_canonico, None);
    assert_eq!(r.ficheros_escritos, 0);
    assert!(r.detalle.contains("ASIMILADO"), "detalle: {}", r.detalle);
}

#[test]
fn la_unificacion_del_runtime_sigue_viva() {
    // MEDIDO (2026-11-23): `unificar_runtime` sigue unificando el binario,
    // pero **FUERA** de `runtime/bin/`: esa carpeta la gestiona la app y su
    // desempaquetador no puede borrar lo que encuentra (`cannot unpack
    // assets/runtime ... cannot delete .../runtime/bin/megadaemon-core`)
    // => el core de Antigravity dejaba de arrancar.
    let e = antigravity_clone::estado_runtime();
    assert!(e.ruta.contains("/megadaemon/bin/"), "ruta: {}", e.ruta);
    assert!(!e.ruta.contains("runtime/bin"), "ruta: {}", e.ruta);
}

// ---------------------------------------------------------------------------
// 6. Paridad MCP
// ---------------------------------------------------------------------------

#[test]
fn el_catalogo_mcp_anuncia_las_tools_nuevas() {
    let cat = megadaemon_core::mcp_server::tool_definitions();
    let arr = cat.as_array().expect("catálogo es una lista");
    let nombres: Vec<&str> = arr
        .iter()
        .filter_map(|d| d.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(nombres.contains(&"antigravity_model"), "catálogo: {nombres:?}");
    assert!(nombres.contains(&"antigravity_env"), "catálogo: {nombres:?}");
}

#[test]
fn las_tools_nuevas_se_despachan() {
    let r = megadaemon_core::mcp_server::dispatch_tool(
        "antigravity_env",
        &json!({"action": "tool", "name": "no-existe-jamas-esta"}),
    );
    assert!(r.get("content").is_some());
    assert!(r.get("structuredContent").is_some());

    // Un nombre desconocido NO lo gestiona este módulo.
    assert!(am::dispatch("herramienta_del_otro_módulo", &json!({})).is_none());
    assert!(ae::dispatch("herramienta_del_otro_módulo", &json!({})).is_none());
}

#[test]
fn agent_coding_llega_a_las_tools_nuevas() {
    // `agent_coding` enruta por `megadaemon_tool` -> `mcp_ext::dispatch`:
    // las tools de asimilación tienen que estar ahí también.
    let r = megadaemon_core::mcp_ext::dispatch(
        "antigravity_env",
        &json!({"action": "tool", "name": "no-existe-jamas-esta"}),
    )
    .expect("la tool debe despacharse");
    assert_eq!(r["ok"], false);
}

#[test]
fn escribir_el_modelo_exige_confirmacion() {
    let r = megadaemon_core::mcp_server::dispatch_tool(
        "antigravity_model",
        &json!({"action": "register"}),
    );
    let sc = r.get("structuredContent").cloned().unwrap_or(json!({}));
    assert_eq!(sc["ok"], false);
    assert_eq!(sc["error"], "confirm_required");
}
