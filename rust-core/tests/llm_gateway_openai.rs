//! MegaDaemon v16 — EL GATEWAY OPENAI-COMPATIBLE (forma exacta, sin red).
//!
//! El gateway es la puerta que usan los clientes externos (Antigravity,
//! cualquier SDK OpenAI…).  Aquí se fija **exactamente** la forma de la
//! respuesta y el comportamiento del enrutado:
//!
//! * la respuesta tiene la forma OpenAI (id/object/created/model/choices[]
//!   /usage) y NADIE más;
//! * `messages` con system/user/assistant se entiende bien;
//! * el contenido en array (multimodal) se entiende;
//! * los mensajes vacíos se RECHAZAN (con un error utilizable);
//! * `model:"megadaemon"` NO ata la petición a un proveedor concreto: es
//!   el router el que elige (y lo dice en `megadaemon.provider_id`);
//! * la traza de la cascada y el estado de cuota viajan en la respuesta;
//! * ninguna credencial sale por la respuesta.
//!
//! Ejecutar (lo compila y testea el root):
//!   cd rust-core && cargo test --test llm_gateway_openai -- --nocapture

mod llm_mock;

use self::llm_mock::{deps, prov, vault, MockTransport, KEY, OK};
use megadaemon_core::llm_gateway::{
    chat_completions_with, gemini_generate_content_with, gemini_prompt, gemini_route,
    parse_messages, MEGADAEMON_ALIASES,
};
use serde_json::json;

/// Vault con dos proveedores y un default.
fn vault_2() -> (megadaemon_core::providers::ProviderVault, MockTransport) {
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, OK);
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "free"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    (v, t)
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. La forma OpenAI, exacta
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn la_respuesta_tiene_exactamente_la_forma_openai() {
    let (v, t) = vault_2();
    let d = deps(&v, &t);
    let body = json!({
        "model": "megadaemon",
        "messages": [{"role": "user", "content": "ping"}]
    });
    let r = chat_completions_with(&body, &d).expect("el gateway debe responder");

    // Campos de primer nivel.
    assert!(r["id"].is_string(), "id: {:?}", r["id"]);
    assert!(
        r["id"].as_str().unwrap().starts_with("chatcmpl-"),
        "id no OpenAI: {:?}",
        r["id"]
    );
    assert_eq!(r["object"], "chat.completion");
    assert!(r["created"].is_u64(), "created: {:?}", r["created"]);
    // HONESTIDAD (2026-11-23): `model` dice el modelo que REALMENTE contestó
    // (el del proveedor que atendió la peticion), no el que pidió el
    // cliente.  Lo pedido viaja en `megadaemon.requested_model`.
    assert_eq!(r["model"], "Proveedor p1");
    assert_eq!(r["megadaemon"]["requested_model"], "megadaemon");
    assert_eq!(r["megadaemon"]["provider_id"], "p1");

    // choices[].
    assert!(r["choices"].is_array());
    assert_eq!(r["choices"].as_array().unwrap().len(), 1);
    assert_eq!(r["choices"][0]["index"], 0);
    assert_eq!(r["choices"][0]["message"]["role"], "assistant");
    assert!(r["choices"][0]["message"]["content"].is_string());
    assert_eq!(r["choices"][0]["message"]["content"], "PONG");
    assert_eq!(r["choices"][0]["finish_reason"], "stop");

    // usage.
    assert!(r["usage"].is_object());
    assert!(r["usage"]["prompt_tokens"].is_u64());
    assert!(r["usage"]["completion_tokens"].is_u64());
    assert!(r["usage"]["total_tokens"].is_u64());

    // La traza propia va en "megadaemon" (no rompe el formato OpenAI).
    assert!(r["megadaemon"].is_object());
    assert_eq!(r["megadaemon"]["provider_id"], "p1");
    assert!(r["megadaemon"]["provider_trace"].is_array());
    assert!(r["megadaemon"].get("quota").is_some());
}

#[test]
fn la_forma_openai_se_conserva_aunque_el_proveedor_falle() {
    // El proveedor falla y el cascado cae al local: la respuesta DEBE
    // seguir siendo OpenAI-compatible (no un 500 opaco).
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 500, r#"{"error":{"message":"boom"}}"#);
    let v = vault(
        vec![prov("p1", "https://p1.test/v1", "m1", "free")],
        "p1",
    );
    let d = deps(&v, &t);
    let body = json!({
        "model": "megadaemon",
        "messages": [{"role": "user", "content": "ping"}]
    });
    let r = chat_completions_with(&body, &d).expect("el gateway debe responder");
    assert_eq!(r["object"], "chat.completion");
    assert!(r["choices"][0]["message"]["content"].is_string());
    assert_eq!(r["megadaemon"]["fallback_used"], true);
    assert_eq!(r["megadaemon"]["provider_id"], "local-seere");
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Los mensajes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn acepta_system_user_assistant() {
    let (v, t) = vault_2();
    let d = deps(&v, &t);
    let body = json!({
        "model": "megadaemon",
        "messages": [
            {"role": "system", "content": "eres un agente"},
            {"role": "user", "content": "hola"},
            {"role": "assistant", "content": "buenas"},
            {"role": "user", "content": "continua"}
        ]
    });
    let r = chat_completions_with(&body, &d).expect("no debe fallar");
    assert_eq!(r["choices"][0]["message"]["role"], "assistant");

    // Lo que salió de verdad hacia el proveedor:
    let enviado = t
        .cuerpo_enviado("https://p1.test/v1")
        .expect("no se envió la petición");
    let mensajes = enviado["messages"].as_array().unwrap();
    assert_eq!(mensajes[0]["role"], "system");
    assert_eq!(mensajes[0]["content"], "eres un agente");
    assert_eq!(mensajes[1]["role"], "user");
    let prompt = mensajes[1]["content"].as_str().unwrap();
    assert!(prompt.contains("hola"), "prompt: {}", prompt);
    assert!(prompt.contains("buenas"), "prompt: {}", prompt);
    assert!(prompt.contains("continua"), "prompt: {}", prompt);
}

#[test]
fn acepta_contenido_en_array() {
    let (v, t) = vault_2();
    let d = deps(&v, &t);
    let body = json!({
        "model": "megadaemon",
        "messages": [
            {"role": "user", "content": [
                {"type": "text", "text": "parte 1"},
                {"type": "text", "text": "parte 2"}
            ]}
        ]
    });
    let r = chat_completions_with(&body, &d).expect("no debe fallar");
    assert_eq!(r["object"], "chat.completion");
    let enviado = t
        .cuerpo_enviado("https://p1.test/v1")
        .expect("no se envió la petición");
    let prompt = enviado["messages"][1]["content"].as_str().unwrap();
    assert!(prompt.contains("parte 1") && prompt.contains("parte 2"));
}

#[test]
fn rechaza_mensajes_vacios() {
    let (v, t) = vault_2();
    let d = deps(&v, &t);
    for body in [
        json!({"model": "megadaemon", "messages": []}),
        json!({"model": "megadaemon"}),
        json!({"model": "megadaemon", "messages": "nope"}),
        json!({"model": "megadaemon", "messages": [{"role": "system", "content": "solo sistema"}]}),
        json!({"model": "megadaemon", "messages": [{"role": "user", "content": "   "}]}),
    ] {
        let r = chat_completions_with(&body, &d);
        assert!(r.is_err(), "debería rechazar: {}", body);
        let e = r.unwrap_err();
        assert!(
            e.status == 400,
            "el error debe ser 400 (petición mala), no {}",
            e.status
        );
        assert!(!e.message.trim().is_empty(), "el error debe tener mensaje");
    }
    // Y no se llamó a NADIE (no se gasta una llamada para rechazar).
    assert!(t.endpoints().is_empty(), "se llamó: {:?}", t.endpoints());
}

#[test]
fn parse_messages_separa_el_sistema_y_el_usuario() {
    let v = json!([
        {"role": "system", "content": "eres un agente"},
        {"role": "user", "content": "hola"}
    ]);
    // v16: `parse_messages` devuelve el struct `ParsedConversation` (antes
    // una tupla `(prompt, system)`): el prompt del usuario es `user_text` y
    // el sistema es `system`.
    let c = parse_messages(&v).unwrap();
    assert_eq!(c.user_text, "hola");
    assert_eq!(c.system.unwrap(), "eres un agente");
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. El enrutado: "megadaemon" NO es un proveedor concreto
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn model_megadaemon_no_ata_la_peticion_a_un_proveedor() {
    for alias in MEGADAEMON_ALIASES {
        let (v, t) = vault_2();
        let d = deps(&v, &t);
        let body = json!({
            "model": alias,
            "messages": [{"role": "user", "content": "ping"}]
        });
        let r = chat_completions_with(&body, &d).expect("no debe fallar");
        // La respuesta dice el modelo REAL que contestó (el del router), y
        // lo pedido por el cliente viaja aparte (honestidad del gateway).
        assert_eq!(r["model"], "Proveedor p1");
        assert_eq!(r["megadaemon"]["requested_model"], *alias);
        // ...pero el que atiende lo decide el ROUTER (el default del
        // vault), no el alias.
        assert_eq!(
            r["megadaemon"]["provider_id"], "p1",
            "el alias debe delegar en el router"
        );
        // Y no se pidió "megadaemon" como si fuera un proveedor.
        assert_eq!(t.veces("https://p1.test/v1"), 1);
    }
}

#[test]
fn un_modelo_concreto_si_se_pide_ese_proveedor() {
    let (v, t) = vault_2();
    let d = deps(&v, &t);
    let body = json!({
        "model": "p2",
        "messages": [{"role": "user", "content": "ping"}]
    });
    let r = chat_completions_with(&body, &d).expect("no debe fallar");
    // El modelo REAL que contestó (honestidad) y el pedido, aparte.
    assert_eq!(r["model"], "Proveedor p2");
    assert_eq!(r["megadaemon"]["requested_model"], "p2");
    assert_eq!(r["megadaemon"]["provider_id"], "p2");
    assert_eq!(t.veces("https://p2.test/v1"), 1);
    assert_eq!(t.veces("https://p1.test/v1"), 0);
}

#[test]
fn el_max_tokens_del_cliente_se_acota() {
    let (v, t) = vault_2();
    let d = deps(&v, &t);
    let body = json!({
        "model": "megadaemon",
        "max_tokens": 9_999_999,
        "messages": [{"role": "user", "content": "ping"}]
    });
    let _ = chat_completions_with(&body, &d).expect("no debe fallar");
    let enviado = t
        .cuerpo_enviado("https://p1.test/v1")
        .expect("no se envió la petición");
    let mt = enviado["max_tokens"].as_u64().expect("max_tokens ausente");
    assert!(mt <= 32_768, "max_tokens sin acotar: {}", mt);
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Ninguna credencial sale por la respuesta
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn ninguna_credencial_sale_por_la_respuesta() {
    let t = MockTransport::new();
    t.responde(
        "https://p1.test/v1",
        401,
        r#"{"error":{"message":"Invalid API key: sk-EXAMPLE-NO-ES-UNA-KEY-0000 (rejected)"}}"#,
    );
    let v = vault(
        vec![prov("p1", "https://p1.test/v1", "m1", "free")],
        "p1",
    );
    let d = deps(&v, &t);
    let body = json!({
        "model": "megadaemon",
        "messages": [{"role": "user", "content": "ping"}]
    });
    let r = chat_completions_with(&body, &d).expect("no debe fallar");
    let texto = serde_json::to_string(&r).unwrap();
    assert!(
        !texto.contains(KEY),
        "la API key salió por la respuesta: {}",
        texto
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. El dialecto Gemini (paridad con Antigravity)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn la_forma_gemini_tiene_candidates() {
    let (v, t) = vault_2();
    let d = deps(&v, &t);
    let body = json!({
        "contents": [{"role": "user", "parts": [{"text": "hola"}]}]
    });
    let r = gemini_generate_content_with(&body, &d).expect("no debe fallar");
    assert!(r["candidates"].is_array());
    assert_eq!(r["candidates"][0]["content"]["role"], "model");
    assert_eq!(r["candidates"][0]["content"]["parts"][0]["text"], "PONG");
    assert_eq!(r["candidates"][0]["finishReason"], "STOP");
    assert_eq!(r["candidates"][0]["index"], 0);
    assert_eq!(r["usageMetadata"]["totalTokenCount"], 7);
    // `modelVersion` dice el modelo REAL que contestó (honestidad).
    assert_eq!(r["modelVersion"], "Proveedor p1");
    assert_eq!(r["megadaemon"]["provider_id"], "p1");
}

#[test]
fn el_dialecto_gemini_rechaza_lo_vacio() {
    assert!(gemini_prompt(&json!({})).is_err());
    assert!(gemini_prompt(&json!({"contents": []})).is_err());
    assert!(gemini_prompt(&json!({"contents": [{"role": "user", "parts": []}]})).is_err());
}

#[test]
fn las_rutas_gemini_son_las_del_language_server() {
    assert_eq!(
        gemini_route("/v1internal:generateContent").unwrap().model,
        ""
    );
    assert!(gemini_route("/v1internal:streamGenerateContent").unwrap().stream);
    assert_eq!(
        gemini_route("/v1beta/models/megadaemon:generateContent")
            .unwrap()
            .model,
        "megadaemon"
    );
    assert!(gemini_route("/v1/chat/completions").is_none());
}

#[test]
fn el_gateway_no_devuelve_el_razonamiento_si_no_hay_que_ello() {
    // El razonamiento del proveedor NO se mete en `content`: va en su
    // campo (`reasoning_content`), y el `content` es la respuesta final.
    let t = MockTransport::new();
    t.responde(
        "https://p1.test/v1",
        200,
        r#"{"choices":[{"index":0,"message":{"role":"assistant","content":"final","reasoning_content":"pensamiento interno"},"finish_reason":"stop"}],"usage":{"total_tokens":3}}"#,
    );
    let v = vault(
        vec![prov("p1", "https://p1.test/v1", "m1", "free")],
        "p1",
    );
    let d = deps(&v, &t);
    let body = json!({
        "model": "megadaemon",
        "messages": [{"role": "user", "content": "ping"}]
    });
    let r = chat_completions_with(&body, &d).expect("no debe fallar");
    assert_eq!(r["choices"][0]["message"]["content"], "final");
    assert_eq!(
        r["choices"][0]["message"]["reasoning_content"],
        "pensamiento interno"
    );
}
