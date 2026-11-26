//! MegaDaemon — AUDITORÍA DEL **BUCLE DE HERRAMIENTAS** (2026-11-26).
//!
//! MEDIDO (2026-11-26, móvil): el language server de Antigravity manda sus
//! `functionDeclarations` y espera una `functionCall` de vuelta.  Antes de
//! este cambio:
//!
//! * el proveedor **nunca** recibía `tools` ⇒ devolvía su protocolo de
//!   TEXTO (`<invoke name="…">…</invoke>`) y el agente la pintaba en vez
//!   de ejecutarla;
//! * `ChatResult` sólo conservaba el NOMBRE de la tool call (`tools_used`)
//!   ⇒ la llamada se perdía por el camino;
//! * la trama SSE se quedaba sólo con el texto ⇒ la `functionCall` se
//!   perdía otra vez al serializar.
//!
//! Suite determinista y **sin red** (el transporte HTTP se inyecta).
//!
//! Ejecutar:
//!   cd rust-core && cargo test --test llm_tools -- --nocapture

mod llm_mock;

use self::llm_mock::{deps, prov, vault, MockTransport};
use megadaemon_core::providers::{
    execute_chat_con_tools_with, execute_chat_with, tool_calls_en_texto,
};
use serde_json::json;

const ENDPOINT: &str = "https://p1.test/v1";

/// Respuesta con `tool_calls` NATIVAS (OpenAI).
fn cuerpo_con_tool_calls() -> String {
    json!({
        "choices": [{
            "index": 0,
            "finish_reason": "tool_calls",
            "message": {
                "role": "assistant",
                "content": "Voy a leerlo.",
                "tool_calls": [{
                    "id": "call_e156d371",
                    "type": "function",
                    "function": {
                        "name": "view_file",
                        "arguments": "{\"AbsolutePath\":\"/tmp/x\"}"
                    }
                }]
            }
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
    })
    .to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Las tools LLEGAN al proveedor
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn las_tools_se_mandan_al_proveedor_solo_cuando_hay_tools() {
    let t = MockTransport::new();
    t.responde(ENDPOINT, 200, &cuerpo_con_tool_calls());
    let v = vault(
        vec![prov("p1", ENDPOINT, "m1", "paid")],
        "p1",
    );
    let d = deps(&v, &t);
    let tools = vec![json!({
        "type": "function",
        "function": {
            "name": "view_file",
            "description": "lee un fichero",
            "parameters": {"type": "object", "properties": {}}
        }
    })];
    let _ = execute_chat_con_tools_with(
        "hola",
        Some("p1"),
        None,
        None,
        128,
        &tools,
        None,
        &d,
    );
    let cuerpo = t.cuerpo_enviado(ENDPOINT).expect("no se envió nada");
    assert_eq!(cuerpo["tools"][0]["function"]["name"], "view_file");
    assert_eq!(cuerpo["tool_choice"], "auto");
}

#[test]
fn sin_tools_no_se_manda_el_campo_tools() {
    let t = MockTransport::new();
    t.responde(ENDPOINT, 200, &cuerpo_con_tool_calls());
    let v = vault(
        vec![prov("p1", ENDPOINT, "m1", "paid")],
        "p1",
    );
    let d = deps(&v, &t);
    let _ = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    let cuerpo = t.cuerpo_enviado(ENDPOINT).expect("no se envió nada");
    assert!(cuerpo.get("tools").is_none(), "no debía mandar tools");
    assert!(cuerpo.get("tool_choice").is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. La tool call ESTRUCTURADA llega al ChatResult
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn la_tool_call_nativa_llega_estructurada_en_el_resultado() {
    let t = MockTransport::new();
    t.responde(ENDPOINT, 200, &cuerpo_con_tool_calls());
    let v = vault(
        vec![prov("p1", ENDPOINT, "m1", "paid")],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(res.tool_calls.len(), 1);
    assert_eq!(res.tool_calls[0].name, "view_file");
    assert_eq!(res.tool_calls[0].id, "call_e156d371");
    assert_eq!(res.tool_calls[0].arguments["AbsolutePath"], "/tmp/x");
    // Y queda constancia en `tools_used` (v16, DEFECTO-08).
    assert!(res.tools_used.iter().any(|s| s == "provider_tool:view_file"));
}

#[test]
fn la_conversacion_lista_se_manda_como_mensajes() {
    let t = MockTransport::new();
    t.responde(ENDPOINT, 200, &cuerpo_con_tool_calls());
    let v = vault(
        vec![prov("p1", ENDPOINT, "m1", "paid")],
        "p1",
    );
    let d = deps(&v, &t);
    let msgs = vec![
        json!({"role": "system", "content": "eres un agente"}),
        json!({"role": "assistant", "content": json!(null),
               "tool_calls": [{"id": "call_1", "type": "function",
                               "function": {"name": "view_file", "arguments": "{}"}}]}),
        json!({"role": "tool", "tool_call_id": "call_1", "content": "{\"contenido\":\"hola\"}"}),
    ];
    let _ = execute_chat_con_tools_with(
        "hola",
        Some("p1"),
        None,
        None,
        128,
        &[],
        Some(&msgs),
        &d,
    );
    let cuerpo = t.cuerpo_enviado(ENDPOINT).expect("no se envió nada");
    let enviados = cuerpo["messages"].as_array().unwrap();
    assert_eq!(enviados.len(), 3);
    assert_eq!(enviados[0]["role"], "system");
    assert_eq!(enviados[1]["tool_calls"][0]["id"], "call_1");
    assert_eq!(enviados[2]["role"], "tool");
    assert_eq!(enviados[2]["tool_call_id"], "call_1");
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. El protocolo de TEXTO de MiniMax (el respaldo)
// ─────────────────────────────────────────────────────────────────────────────

/// Texto REAL capturado en el móvil (2026-11-26): MiniMax, sin `tools`,
/// deja la llamada en el texto con su envoltorio de tokens.
fn texto_de_minimax() -> String {
    let mut s = String::new();
    s.push_str("Voy a leer el fichero.<]\u{200b}minimax[>[<\u{200b}tool_call>\n");
    s.push_str("]<]\u{200b}minimax[>[<invoke name=\"view_file\">\n");
    s.push_str("{\"AbsolutePath\":\"/tmp/x\",\"StartLine\":1}");
    s.push_str("</invoke>\n");
    s.push_str("We need respond to current user?");
    s
}

#[test]
fn la_tool_call_en_texto_se_recupera() {
    let texto = texto_de_minimax();
    let (calls, limpio) = tool_calls_en_texto(&texto);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "view_file");
    assert_eq!(calls[0].arguments["AbsolutePath"], "/tmp/x");
    assert_eq!(calls[0].arguments["StartLine"], 1);
    // El texto útil se conserva; los marcadores desaparecen.
    assert!(limpio.contains("Voy a leer el fichero."));
    assert!(!limpio.contains("invoke name="));
    assert!(!limpio.contains("\u{200b}"));
}

#[test]
fn un_texto_sin_llamadas_no_se_toca() {
    let texto = "Una respuesta normal con { llaves } y comillas \"normales\".";
    let (calls, limpio) = tool_calls_en_texto(texto);
    assert!(calls.is_empty());
    assert_eq!(limpio, texto);
}

#[test]
fn la_tool_call_en_texto_llega_al_resultado() {
    let t = MockTransport::new();
    t.responde(
        ENDPOINT,
        200,
        &json!({
            "choices": [{"index": 0, "finish_reason": "stop",
                         "message": {"role": "assistant", "content": texto_de_minimax()}}]
        })
        .to_string(),
    );
    let v = vault(
        vec![prov("p1", ENDPOINT, "m1", "paid")],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(res.tool_calls.len(), 1);
    assert_eq!(res.tool_calls[0].name, "view_file");
    assert!(!res.reply.contains("invoke name="));
}
