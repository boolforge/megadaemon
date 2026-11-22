//! Suite del **gateway OpenAI-compatible** de MegaDaemon (v16).
//!
//! Determinista y **SIN RED**: el vault, el transporte y el motor local se
//! inyectan.
//!
//! Qué se comprueba:
//!   * la respuesta tiene **EXACTAMENTE** la forma OpenAI;
//!   * acepta mensajes `system` / `user` / `assistant`;
//!   * acepta `content` como array (multimodal);
//!   * rechaza mensajes vacíos con **400**;
//!   * `model: "megadaemon"` NO llama a un proveedor concreto (routing
//!     completo por el modelo por defecto de la boveda);
//!   * el razonamiento y la trazabilidad son **opt-in**.

use megadaemon_core::llm_gateway::{
    chat_completions_with_vault, content_to_text, parse_messages, GatewayError,
};
use megadaemon_core::providers::{
    ChatPolicy, LlmRequest, LlmResponse, LlmTransport, LocalFallback, ProviderEntry,
    ProviderError, ProviderVault,
};
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// Transportes de mentira
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct MockTransport {
    calls: Mutex<Vec<(String, String, Value)>>,
}

impl MockTransport {
    fn new() -> MockTransport {
        MockTransport::default()
    }

    fn endpoints(&self) -> Vec<String> {
        self.calls
            .lock()
            .map(|c| c.iter().map(|(e, _, _)| e.clone()).collect())
            .unwrap_or_default()
    }

    fn bodies(&self) -> Vec<Value> {
        self.calls
            .lock()
            .map(|c| c.iter().map(|(_, _, b)| b.clone()).collect())
            .unwrap_or_default()
    }
}

impl LlmTransport for MockTransport {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn post(&self, req: &LlmRequest, _timeout: Duration) -> Result<LlmResponse, ProviderError> {
        if let Ok(mut c) = self.calls.lock() {
            c.push((req.endpoint.clone(), req.model.clone(), req.body.clone()));
        }
        let body = json!({
            "id": "chatcmpl-gw",
            "object": "chat.completion",
            "created": 1764000000,
            "model": "test-model",
            "choices": [{"index":0,"message":{"role":"assistant","content":"RESPUESTA"},"finish_reason":"stop"}],
            "usage": {"prompt_tokens": 11, "completion_tokens": 7, "total_tokens": 18}
        })
        .to_string();
        Ok(LlmResponse { status: 200, body })
    }
}

#[derive(Debug, Default)]
struct StubLocal;

impl LocalFallback for StubLocal {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn run(&self, _prompt: &str, _max_turns: usize) -> Value {
        json!({"answer": "LOCAL", "tools": [], "tokens": 3})
    }
}

fn provider(id: &str, endpoint: &str, tier: &str) -> ProviderEntry {
    ProviderEntry {
        id: id.to_string(),
        name: format!("Provider {}", id),
        vendor: "Test".to_string(),
        endpoint: endpoint.to_string(),
        model: format!("model-{}", id),
        api_key: "key".to_string(),
        auth_type: "bearer".to_string(),
        tier: tier.to_string(),
        free_signup_url: String::new(),
        enabled: true,
        supports_reasoning: false,
    }
}

const EP_DEFAULT: &str = "https://default.test/v1/chat/completions";
const EP_OTHER: &str = "https://other.test/v1/chat/completions";

fn vault() -> ProviderVault {
    ProviderVault {
        default_model: "default-free".to_string(),
        providers: vec![
            provider("default-free", EP_DEFAULT, "free"),
            provider("other-free", EP_OTHER, "free"),
        ],
    }
}

fn policy() -> ChatPolicy {
    megadaemon_core::providers::ChatPolicy::for_tests(
        megadaemon_core::service_bus::Budget::new(1.0, 10.0),
    )
}

fn call(body: &Value, t: &MockTransport) -> Result<Value, GatewayError> {
    chat_completions_with_vault(body, &vault(), t, &StubLocal, &policy())
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Forma OpenAI exacta
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn la_respuesta_tiene_exactamente_la_forma_openai() {
    let t = MockTransport::new();
    let body = json!({
        "model": "megadaemon",
        "messages": [{"role": "user", "content": "hola"}]
    });
    let v = call(&body, &t).expect("el gateway debe responder");

    // Las claves de primer nivel son EXACTAMENTE las de OpenAI.
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(|k| k.as_str()).collect();
    keys.sort();
    assert_eq!(
        keys,
        vec!["choices", "created", "id", "model", "object", "usage"],
        "la respuesta debe ser OpenAI pura (sin extensiones no pedidas)"
    );

    assert_eq!(v["object"], "chat.completion");
    assert!(v["id"].as_str().unwrap().starts_with("chatcmpl-"));
    assert!(v["created"].as_u64().unwrap() > 1_600_000_000);

    let choice = &v["choices"][0];
    assert_eq!(choice["index"], 0);
    assert_eq!(choice["message"]["role"], "assistant");
    assert!(choice["message"]["content"].is_string());
    assert_eq!(choice["finish_reason"], "stop");

    let usage = &v["usage"];
    assert!(usage["prompt_tokens"].is_u64());
    assert!(usage["completion_tokens"].is_u64());
    assert!(usage["total_tokens"].is_u64());
    assert_eq!(
        usage["total_tokens"].as_u64().unwrap(),
        usage["prompt_tokens"].as_u64().unwrap() + usage["completion_tokens"].as_u64().unwrap()
    );
}

#[test]
fn el_modelo_de_la_respuesta_es_el_pedido() {
    let t = MockTransport::new();
    let body = json!({
        "model": "megadaemon",
        "messages": [{"role": "user", "content": "hola"}]
    });
    let v = call(&body, &t).unwrap();
    assert_eq!(v["model"], "megadaemon");
}

#[test]
fn la_trazabilidad_es_opcional() {
    let t = MockTransport::new();
    let body = json!({
        "model": "megadaemon",
        "messages": [{"role": "user", "content": "hola"}]
    });
    let v = call(&body, &t).unwrap();
    assert!(
        v.get("megadaemon").is_none(),
        "sin include_trace no debe haber bloque de trazabilidad"
    );

    let mut body2 = body.clone();
    body2["include_trace"] = json!(true);
    let v2 = call(&body2, &t).unwrap();
    assert!(v2.get("megadaemon").is_some(), "include_trace debe traerlo");
    assert_eq!(v2["megadaemon"]["provider_id"], "default-free");
    assert!(v2["megadaemon"]["latency_ms"].is_number());
    assert_eq!(v2["megadaemon"]["fallback_used"], false);
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Mensajes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn acepta_mensajes_system_user_assistant() {
    let t = MockTransport::new();
    let body = json!({
        "model": "megadaemon",
        "messages": [
            {"role": "system", "content": "eres un agente"},
            {"role": "user", "content": "hola"},
            {"role": "assistant", "content": "buenas"},
            {"role": "user", "content": "mas cosas"}
        ]
    });
    let v = call(&body, &t).unwrap();
    assert_eq!(v["object"], "chat.completion");
    // El proveedor recibe los roles INTACTOS.
    let sent = t.bodies();
    let msgs = sent[0]["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 4);
    assert_eq!(msgs[0]["role"], "system");
    assert_eq!(msgs[1]["role"], "user");
    assert_eq!(msgs[2]["role"], "assistant");
    assert_eq!(msgs[3]["role"], "user");
}

#[test]
fn acepta_content_como_array() {
    let t = MockTransport::new();
    let body = json!({
        "model": "megadaemon",
        "messages": [{"role": "user", "content": [
            {"type": "text", "text": "parte 1"},
            {"type": "text", "text": "parte 2"}
        ]}]
    });
    let v = call(&body, &t).unwrap();
    assert_eq!(v["object"], "chat.completion");
    let sent = t.bodies();
    let msgs = sent[0]["messages"].as_array().unwrap();
    assert!(msgs[0]["content"].as_str().unwrap().contains("parte 1"));
    assert!(msgs[0]["content"].as_str().unwrap().contains("parte 2"));
}

#[test]
fn rechaza_mensajes_vacios_con_400() {
    let t = MockTransport::new();
    for body in [
        json!({"model": "megadaemon", "messages": []}),
        json!({"model": "megadaemon"}),
        json!({"model": "megadaemon", "messages": "nope"}),
        json!({"model": "megadaemon", "messages": [{"role": "system", "content": "solo system"}]}),
        json!({"model": "megadaemon", "messages": [{"role": "user", "content": "   "}]}),
    ] {
        let e = call(&body, &t).expect_err("debe rechazar");
        assert_eq!(e.status, 400, "cuerpo: {:?}", body);
        assert!(!e.message.is_empty());
        // Y el cuerpo de error tiene la forma de OpenAI.
        let j = e.to_json();
        assert!(j["error"]["message"].is_string());
        assert_eq!(j["error"]["code"], 400);
    }
    assert_eq!(t.calls.lock().unwrap().len(), 0, "no se debe llamar a nadie");
}

#[test]
fn el_error_de_gateway_no_filtra_credenciales() {
    let e = GatewayError::bad("algo fallo");
    let j = serde_json::to_string(&e.to_json()).unwrap();
    assert!(!j.contains("sk-"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Routing
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn model_megadaemon_no_llama_a_un_proveedor_concreto() {
    let t = MockTransport::new();
    let body = json!({
        "model": "megadaemon",
        "messages": [{"role": "user", "content": "hola"}]
    });
    let v = call(&body, &t).unwrap();
    // Se usa el modelo POR DEFECTO de la boveda (routing completo), no uno
    // fijo elegido por el gateway.
    assert_eq!(
        t.endpoints(),
        vec![EP_DEFAULT.to_string()],
        "model=megadaemon debe usar el default de la boveda"
    );
    // Y el proveedor recibe SU modelo, no "megadaemon".
    let sent = t.bodies();
    assert_eq!(sent[0]["model"], "model-default-free");
    assert_eq!(v["model"], "megadaemon", "la respuesta hace eco del modelo pedido");
}

#[test]
fn un_alias_tambien_hace_routing_completo() {
    for alias in ["megadaemon", "auto", "default", "MEGADAEMON", "megadaemon/router"] {
        let t = MockTransport::new();
        let body = json!({
            "model": alias,
            "messages": [{"role": "user", "content": "hola"}]
        });
        let _ = call(&body, &t).unwrap();
        assert_eq!(
            t.endpoints(),
            vec![EP_DEFAULT.to_string()],
            "el alias '{}' debe hacer routing completo",
            alias
        );
    }
}

#[test]
fn un_id_concreto_llama_a_ese_proveedor() {
    let t = MockTransport::new();
    let body = json!({
        "model": "other-free",
        "messages": [{"role": "user", "content": "hola"}]
    });
    let v = call(&body, &t).unwrap();
    assert_eq!(t.endpoints(), vec![EP_OTHER.to_string()]);
    assert_eq!(v["model"], "other-free");
}

#[test]
fn el_gateway_reutiliza_el_fallback_del_daemon() {
    // Si el proveedor pedido falla, el gateway cae al siguiente (el fallback
    // real de `providers::execute_chat_in`).
    #[derive(Debug, Default)]
    struct FailingThenOk {
        calls: Mutex<usize>,
    }
    impl LlmTransport for FailingThenOk {
        fn post(
            &self,
            req: &LlmRequest,
            _t: Duration,
        ) -> Result<LlmResponse, ProviderError> {
            let mut c = self.calls.lock().unwrap();
            *c += 1;
            if req.endpoint == EP_DEFAULT {
                return Ok(LlmResponse {
                    status: 500,
                    body: "{\"error\":{\"message\":\"boom\"}}".into(),
                });
            }
            Ok(LlmResponse {
                status: 200,
                body: json!({
                    "id": "x", "object": "chat.completion", "created": 1, "model": "m",
                    "choices": [{"index":0,"message":{"role":"assistant","content":"OK_FALLBACK"},"finish_reason":"stop"}],
                    "usage": {"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}
                })
                .to_string(),
            })
        }
    }
    let t = FailingThenOk::default();
    let body = json!({
        "model": "megadaemon",
        "messages": [{"role": "user", "content": "hola"}]
    });
    let v = chat_completions_with_vault(&body, &vault(), &t, &StubLocal, &policy()).unwrap();
    assert_eq!(v["choices"][0]["message"]["content"], "OK_FALLBACK");
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Razonamiento
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn el_razonamiento_no_se_filtra_por_defecto() {
    #[derive(Debug, Default)]
    struct WithReasoning;
    impl LlmTransport for WithReasoning {
        fn post(
            &self,
            _req: &LlmRequest,
            _t: Duration,
        ) -> Result<LlmResponse, ProviderError> {
            Ok(LlmResponse {
                status: 200,
                body: json!({
                    "id": "x", "object": "chat.completion", "created": 1, "model": "m",
                    "choices": [{"index":0,"message":{
                        "role":"assistant","content":"RESPUESTA",
                        "reasoning_content":"PENSAMIENTO SECRETO DEL MODELO"
                    },"finish_reason":"stop"}],
                    "usage": {"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}
                })
                .to_string(),
            })
        }
    }
    let body = json!({"model": "megadaemon", "messages": [{"role":"user","content":"hola"}]});
    let v = chat_completions_with_vault(&body, &vault(), &WithReasoning, &StubLocal, &policy())
        .unwrap();
    let txt = serde_json::to_string(&v).unwrap();
    assert!(
        !txt.contains("PENSAMIENTO SECRETO"),
        "el razonamiento no debe filtrarse al cliente"
    );

    let mut body2 = body.clone();
    body2["include_reasoning"] = json!(true);
    let v2 = chat_completions_with_vault(&body2, &vault(), &WithReasoning, &StubLocal, &policy())
        .unwrap();
    assert_eq!(
        v2["choices"][0]["message"]["reasoning_content"],
        "PENSAMIENTO SECRETO DEL MODELO"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Utilidades del parser
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_messages_conserva_los_roles() {
    let v = json!([
        {"role":"system","content":"sys"},
        {"role":"user","content":"u1"},
        {"role":"assistant","content":"a1"},
        {"role":"user","content":"u2"}
    ]);
    let c = parse_messages(&v).unwrap();
    assert_eq!(c.system.as_deref(), Some("sys"));
    assert_eq!(c.user_text, "u1\n\nu2");
    assert_eq!(c.messages.len(), 4);
    assert_eq!(c.messages[2]["role"], "assistant");
}

#[test]
fn content_to_text_acepta_string_y_array() {
    assert_eq!(content_to_text(Some(&json!("hola"))), "hola");
    assert_eq!(
        content_to_text(Some(&json!([{"type":"text","text":"a"},{"type":"text","text":"b"}]))),
        "a\nb"
    );
    assert_eq!(content_to_text(None), "");
    assert_eq!(content_to_text(Some(&json!(42))), "");
}

#[test]
fn el_gateway_no_puede_leer_una_key_del_vault() {
    // El vault inyectado tiene la key "key"; la respuesta no debe incluirla.
    let t = MockTransport::new();
    let body = json!({"model":"megadaemon","messages":[{"role":"user","content":"hola"}]});
    let v = call(&body, &t).unwrap();
    let txt = serde_json::to_string(&v).unwrap();
    assert!(!txt.contains("\"key\""), "la respuesta no debe llevar credenciales");
}
