//! Smoke test del puente contra la IDE agéntica **Antigravity** (`antigravity_bridge`).
//!
//! El módulo vive en la libreria (`use megadaemon_core::...`) porque dentro
//! de un arnés `crate::` es la raíz del ARNÉS: un módulo copiado con
//! `#[path = ...]` no resolvería `crate::antigravity_intel` (E0433) y el
//! target de test **no compilaría** ⇒ la suite entera moriría sin ejecutar
//! ni un test.
//!
//! Qué se prueba (todo **puro**, sin red):
//!   * el parser del `__APP_CONFIG__` del SPA (y que **no** se trague un
//!     `{` dentro de una cadena);
//!   * el parser de errores Connect (`{"code","message"}`);
//!   * el lector del modelo del `antigravity_state.pbtxt`;
//!   * el catálogo de tools MCP en disco;
//!   * el resumen de conversaciones y de pasos;
//!   * la URL del método Connect (servicio + método);
//!   * que **ningún** camino del módulo imprima el token CSRF.

use std::fs;
use std::path::PathBuf;

use megadaemon_core::antigravity_bridge as ab;
use megadaemon_core::antigravity_bridge::{
    app_config_from_html, conversation_summaries_from, csrf_from_html, model_from_state,
    parse_connect_error, parse_tool_definition, step_summaries_from, tool_catalog_from_dir,
    BridgeConfig, BridgeError,
};
use serde_json::json;

fn tmp_dir(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("agy-bridge-smoke-{}-{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

/// HTML real del SPA del móvil (MEDIDO 2026-11-22).
const SPA: &str = r#"<!doctype html>
<html lang="en"><head><title>Jetski Web</title></head><body>
<script>window.__APP_CONFIG__ = {"productName":"antigravity-app","csrfToken":"f29b6cc2-d1c5-4627-bcb1-18c7537f2f5e","appVersion":"2.11.0","releaseChannel":"stable","devMode":false};</script>
</body></html>"#;

#[test]
fn app_config_se_parssea_del_spa() {
    let cfg = app_config_from_html(SPA).expect("debe haber __APP_CONFIG__");
    assert_eq!(cfg["productName"], "antigravity-app");
    assert_eq!(cfg["appVersion"], "2.11.0");
    let tok = csrf_from_html(SPA).expect("csrfToken");
    assert_eq!(tok.len(), 36);
}

#[test]
fn app_config_aguanta_llaves_dentro_de_cadenas() {
    let html = r#"<script>window.__APP_CONFIG__ = {"csrfToken":"abc","nota":"un } y una { dentro","x":1};</script>"#;
    let cfg = app_config_from_html(html).expect("config");
    assert_eq!(cfg["csrfToken"], "abc");
    assert_eq!(cfg["x"], 1);
}

#[test]
fn sin_config_no_hay_token() {
    assert!(app_config_from_html("<html><body>nada</body></html>").is_none());
    assert!(csrf_from_html("<html>nada</html>").is_none());
}

#[test]
fn error_connect_se_lee_con_codigo_y_mensaje() {
    let (code, msg) = parse_connect_error(
        r#"{"code":"invalid_argument","message":"invalid_argument: conversation_id is required (error ID: 5768828f5e6248bfaa0567ef2b3bf12f)"}"#,
    )
    .expect("error connect");
    assert_eq!(code, "invalid_argument");
    assert!(msg.contains("conversation_id is required"));
    assert!(parse_connect_error("no json").is_none());
}

#[test]
fn modelo_se_lee_del_estado_pbtxt() {
    let st = "post_onboarding: {\n  completed_steps: X\n}\nlast_selected_agent_model: MODEL_PLACEHOLDER_M319\nmigrations: {\n  key: 3\n}\n";
    assert_eq!(
        model_from_state(st).as_deref(),
        Some("MODEL_PLACEHOLDER_M319")
    );
    assert!(model_from_state("nada aqui").is_none());
}

#[test]
fn catalogo_de_tools_se_lee_de_disco() {
    let d = tmp_dir("tools");
    let srv = d.join("megadaemon-minimax");
    fs::create_dir_all(&srv).unwrap();
    fs::write(
        srv.join("fs_write.json"),
        r#"{"name":"fs_write","description":"Escribe un fichero.\nSegunda linea.","parameters":{"properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"],"type":"object"}}"#,
    )
    .unwrap();
    fs::write(srv.join("root_exec.json"), r#"{"name":"root_exec"}"#).unwrap();
    fs::write(srv.join("nota.md"), "esto no es una tool").unwrap();

    let cat = tool_catalog_from_dir(&d);
    assert_eq!(cat.len(), 2);
    assert_eq!(cat[0].server, "megadaemon-minimax");
    assert_eq!(cat[0].name, "fs_write");
    assert_eq!(cat[0].params, 2);
    assert_eq!(cat[0].description, "Escribe un fichero.");
    assert_eq!(cat[1].name, "root_exec");
}

#[test]
fn tool_definition_invalida_no_revienta() {
    assert!(parse_tool_definition("s", "no json").is_none());
    assert!(parse_tool_definition("s", "{}").is_none());
}

#[test]
fn conversaciones_se_resumen_desde_el_mapa_real() {
    // Forma REAL medida el 2026-11-22.
    let v = json!({
        "trajectorySummaries": {
            "9dfa200c-b4cf-408f-9f67-d9bee2bd61a4": {
                "summary": "ScummVM Porting Snatcher PSX Saturn",
                "stepCount": 20560,
                "lastModifiedTime": "2026-11-21T20:50:44.139186689Z",
                "trajectoryId": "20068b68-34d2-4f10-9ef3-ead34f74ed7f",
                "status": "CASCADE_RUN_STATUS_IDLE",
                "createdTime": "2026-11-17T02:01:37.514471620Z",
                "workspaces": [{"workspaceFolderAbsoluteUri": "file:///data/user/0/com.agy/files/dumper-web"}]
            }
        }
    });
    let cs = conversation_summaries_from(&v);
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].cascade_id, "9dfa200c-b4cf-408f-9f67-d9bee2bd61a4");
    assert_eq!(cs[0].step_count, 20560);
    assert_eq!(cs[0].status, "CASCADE_RUN_STATUS_IDLE");
    assert_eq!(cs[0].workspaces.len(), 1);
    assert!(conversation_summaries_from(&json!({})).is_empty());
}

#[test]
fn pasos_se_resumen_con_texto_y_error() {
    let v = json!({
        "trajectory": {
            "cascadeId": "c1",
            "steps": [
                {"type": "CORTEX_STEP_TYPE_USER_INPUT",
                 "metadata": {"status": "CORTEX_STEP_STATUS_DONE"},
                 "userInput": {"items": [{"text": "hola"}], "userResponse": "hola"}},
                {"type": "CORTEX_STEP_TYPE_ERROR_MESSAGE",
                 "metadata": {"status": "CORTEX_STEP_STATUS_DONE"},
                 "errorMessage": {"error": {"shortError": "boom"}}},
                {"type": "CORTEX_STEP_TYPE_PLANNER_RESPONSE",
                 "metadata": {"status": "CORTEX_STEP_STATUS_DONE"},
                 "plannerResponse": {"thinking": "pienso", "response": "respuesta"}}
            ]
        },
        "status": "CASCADE_RUN_STATUS_IDLE"
    });
    let s = step_summaries_from(&v);
    assert_eq!(s.len(), 3);
    assert_eq!(s[0].kind, "CORTEX_STEP_TYPE_USER_INPUT");
    assert_eq!(s[0].status, "CORTEX_STEP_STATUS_DONE");
    assert_eq!(s[1].error, "boom");
    assert_eq!(s[2].text, "respuesta");
}

#[test]
fn url_del_metodo_es_el_servicio_connect() {
    let cfg = BridgeConfig::default();
    assert_eq!(cfg.base(), "https://127.0.0.1:45157");
    let b = ab::AntigravityBridge::with_csrf(
        &BridgeConfig {
            host: "127.0.0.1".into(),
            port: 45157,
            ..BridgeConfig::default()
        },
        "tok",
    )
    .expect("cliente");
    assert_eq!(
        b.method_url("SearchConversations"),
        "https://127.0.0.1:45157/exa.language_server_pb.LanguageServerService/SearchConversations"
    );
}

#[test]
fn el_csrf_solo_sale_como_sha16() {
    let b = ab::AntigravityBridge::with_csrf(&BridgeConfig::default(), "secreto-de-prueba")
        .expect("cliente");
    let h = b.csrf_sha16();
    assert_eq!(h.len(), 16);
    assert!(!h.contains("secreto"));
    // El sha16 NO es el token.
    assert_ne!(h, "secreto-de-prueba");
}

#[test]
fn error_http_conserva_codigo_y_mensaje_literal() {
    let e = BridgeError::Http {
        status: 400,
        code: "invalid_argument".into(),
        message: "invalid_argument: conversation_id is required".into(),
    };
    let s = e.to_string();
    assert!(s.contains("400"));
    assert!(s.contains("invalid_argument"));
    assert!(s.contains("conversation_id is required"));
}

#[test]
fn el_texto_del_fuente_no_tiene_el_token_de_ejemplo() {
    // Guarda: en el repo no puede aparecer un token real.  Este test
    // comprueba que el módulo no lo ha incrustado en ningún sitio.
    let src = include_str!("../src/antigravity_bridge.rs");
    assert!(!src.contains("f29b6cc2-d1c5-4627-bcb1-18c7537f2f5e"));
}
