//! MegaDaemon v16 — AUDITORÍA PROFUNDA DE LA PARTE DE CHAT/LLM (cascada).
//!
//! Suite **determinista y SIN RED**: el transporte HTTP se INYECTA
//! (`providers::LlmTransport`), así que cada caso de la cascada se puede
//! probar sin tocar la red y sin credenciales.
//!
//! Qué se fija aquí (todo con su DEFECTO en docs/AUDIT_LLM_V16.md):
//!
//! * la cascada elige el proveedor pedido;
//! * cae al siguiente si el primero falla;
//! * el SEERE local es el ÚLTIMO recurso;
//! * `fallback_used` dice la verdad;
//! * un proveedor RETIRADO no se usa en silencio (ni pedido ni de rebote);
//! * el presupuesto bloquea la llamada cara;
//! * en `pausa` de cuota la cascada salta al siguiente;
//! * un 401 NO se reintenta; un 429/5xx SI;
//! * la API key del proveedor NO puede acabar en la respuesta del cliente;
//! * `tools_used` refleja las tool calls nativas;
//! * `max_tokens` sale acotado y el timeout se pasa al transporte;
//! * PROPIEDAD: para N respuestas capturadas (y para TODOS sus cortes) el
//!   parseo nunca entra en panic y siempre devuelve algo o un error con
//!   mensaje útil.
//!
//! Ejecutar (lo compila y testea el root, ver docs/PROTOCOLO_FLOCKA.md):
//!   cd rust-core && cargo test --test llm_routing -- --nocapture

mod fixtures;
mod llm_mock;

use self::llm_mock::{deps, prov, vault, MockTransport, KEY, OK};
use megadaemon_core::providers::{
    execute_chat_con_tools_with, execute_chat_with, parse_anthropic_reply, parse_provider_error,
    parse_provider_error_con_retry, parse_provider_reply, redact_secret, truncar, validar_endpoint,
    ChatResult, ProviderEntry, MAX_TOKENS_CAP,
};
use serde_json::Value;
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// 1. La cascada
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn la_cascada_elige_el_proveedor_pedido() {
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, OK);
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(res.model_id, "p1", "no atendió el proveedor pedido");
    assert!(!res.fallback_used, "fallback_used debe ser false");
    assert_eq!(res.reply, "PONG");
    // Sólo se llamó al pedido: la cascada no tiene por qué ir a buscar a
    // los demás.
    assert_eq!(t.endpoints(), vec!["https://p1.test/v1".to_string()]);
}

#[test]
fn cae_al_siguiente_si_el_primero_falla() {
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 401, r#"{"error":{"message":"Invalid API key"}}"#);
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(res.model_id, "p2", "no cayó al siguiente proveedor");
    assert!(res.fallback_used, "fallback_used debe ser true");
    assert!(
        res.provider_trace.iter().any(|l| l.contains("p1") && l.contains("401")),
        "la traza debe decir que p1 falló con 401: {:?}",
        res.provider_trace
    );
}

#[test]
fn el_seere_local_es_el_ultimo_recurso() {
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 500, r#"{"error":{"message":"boom"}}"#);
    t.responde("https://p2.test/v1", 401, r#"{"error":{"message":"Invalid API key"}}"#);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(res.model_id, "local-seere", "el local debe ser el último recurso");
    assert!(res.fallback_used);
    assert!(
        res.reply.contains("Fallback offline activo"),
        "la respuesta debe avisar del fallback offline: {}",
        res.reply
    );
}

#[test]
fn fallback_used_dice_la_verdad() {
    // (a) el primero responde => false
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, OK);
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(res.model_id, "p1");
    assert!(!res.fallback_used);

    // (b) el segundo responde => true
    let t2 = MockTransport::new();
    t2.responde("https://p1.test/v1", 401, r#"{"error":{"message":"nope"}}"#);
    t2.responde("https://p2.test/v1", 200, OK);
    let v2 = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let d2 = deps(&v2, &t2);
    let res2 = execute_chat_with("hola", Some("p1"), None, None, 128, &d2);
    assert_eq!(res2.model_id, "p2");
    assert!(res2.fallback_used);
}

#[test]
fn un_proveedor_retirado_no_se_usa_en_silencio() {
    // (a) pedido explícito: aviso, sin llamada
    let t = MockTransport::new();
    let v = vault(
        vec![
            prov("minimax-m2.7", "https://m27.test/v1", "MiniMax-M2.7", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p2",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("minimax-m2.7"), None, None, 128, &d);
    assert!(
        res.reply.contains("MODELO RETIRADO"),
        "debe avisar del retiro: {}",
        res.reply
    );
    assert!(!res.fallback_used, "no debe haber sustituido en silencio");
    assert!(
        t.endpoints().is_empty(),
        "no debe llamar a un proveedor retirado: {:?}",
        t.endpoints()
    );

    // (b) de rebote en la cascada: se SALTA (y se ve en la traza)
    let t2 = MockTransport::new();
    t2.responde("https://p1.test/v1", 401, r#"{"error":{"message":"nope"}}"#);
    t2.responde("https://p2.test/v1", 200, OK);
    let v2 = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            prov("minimax-m2.7", "https://m27.test/v1", "MiniMax-M2.7", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let d2 = deps(&v2, &t2);
    let res2 = execute_chat_with("hola", Some("p1"), None, None, 128, &d2);
    assert_eq!(res2.model_id, "p2");
    assert_eq!(t2.veces("https://m27.test/v1"), 0, "se llamó a un retirado");
    assert!(
        res2.provider_trace
            .iter()
            .any(|l| l.contains("minimax-m2.7") && l.contains("retirado")),
        "la traza debe avisar del salto por retirement: {:?}",
        res2.provider_trace
    );
}

#[test]
fn el_presupuesto_bloquea_la_llamada_cara() {
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, OK);
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let mut d = deps(&v, &t);
    // El presupuesto veta el proveedor de PAGO.
    let sin_pago = |p: &ProviderEntry, _t: u32| p.tier != "paid";
    d.budget_permite = Some(&sin_pago);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(t.veces("https://p1.test/v1"), 0, "se llamó al proveedor caro");
    assert_eq!(res.model_id, "p2", "debe atender el gratuito");
    assert!(
        res.provider_trace
            .iter()
            .any(|l| l.contains("presupuesto")),
        "la traza debe decir que se saltó por presupuesto: {:?}",
        res.provider_trace
    );
}

#[test]
fn en_pausa_de_cuota_la_cascada_salta_al_siguiente() {
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, OK);
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let mut d = deps(&v, &t);
    d.quota_paused = |id: &str| id == "p1";
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(t.veces("https://p1.test/v1"), 0, "se llamó a un proveedor en pausa");
    assert_eq!(res.model_id, "p2");
    assert!(
        res.provider_trace
            .iter()
            .any(|l| l.contains("pausa")),
        "la traza debe decir que se saltó por cuota en pausa: {:?}",
        res.provider_trace
    );
}

#[test]
fn un_4xx_no_se_reintenta_y_un_429_si() {
    // 401 => NO se reintenta (1 sola llamada)
    let t = MockTransport::new();
    t.responde_n("https://p1.test/v1", 401, r#"{"error":{"message":"nope"}}"#, 5);
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let mut d = deps(&v, &t);
    d.retries = 2;
    d.backoff_ms = 0;
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(t.veces("https://p1.test/v1"), 1, "un 401 no se reintenta");
    assert_eq!(res.model_id, "p2");

    // 429 => SI se reintenta (1 + 2 reintentos)
    let t2 = MockTransport::new();
    t2.responde_n(
        "https://p1.test/v1",
        429,
        r#"{"error":{"message":"rate limit reached; resets at 1795130605"}}"#,
        5,
    );
    t2.responde("https://p2.test/v1", 200, OK);
    let v2 = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let mut d2 = deps(&v2, &t2);
    d2.retries = 2;
    d2.backoff_ms = 0;
    let res2 = execute_chat_with("hola", Some("p1"), None, None, 128, &d2);
    assert_eq!(t2.veces("https://p1.test/v1"), 3, "un 429 debe reintentarse 2 veces");
    assert_eq!(res2.model_id, "p2");
}

#[test]
fn la_api_key_del_proveedor_no_llega_al_cliente() {
    let t = MockTransport::new();
    // El cuerpo de error ECOA la clave (caso real de algunos proveedores).
    t.responde(
        "https://p1.test/v1",
        401,
        r#"{"error":{"message":"Invalid API key: sk-EXAMPLE-NO-ES-UNA-KEY-0000 (rejected)"}}"#,
    );
    let v = vault(
        vec![prov("p1", "https://p1.test/v1", "m1", "paid")],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert!(
        !res.reply.contains(KEY),
        "la API key llegó al cliente: {}",
        res.reply
    );
    let traza = res.provider_trace.join(" | ");
    assert!(
        !traza.contains(KEY),
        "la API key llegó a la traza: {}",
        traza
    );
    // Pero el diagnóstico sigue siendo útil.
    assert!(
        traza.contains("401"),
        "la traza debe decir el status: {}",
        traza
    );
}

#[test]
fn tools_used_refleja_las_tool_calls_nativas() {
    let t = MockTransport::new();
    t.responde(
        "https://p1.test/v1",
        200,
        &fixtures::cuerpo("openrouter-free__tool_calls.json"),
    );
    let v = vault(
        vec![prov("p1", "https://p1.test/v1", "m1", "free")],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(res.model_id, "p1");
    assert!(
        res.tools_used.iter().any(|t| t == "provider_tool:leer_fichero"),
        "tools_used debe reflejar la tool nativa: {:?}",
        res.tools_used
    );
}

#[test]
fn max_tokens_sale_acotado_y_el_timeout_se_pasa_al_transporte() {
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, OK);
    let v = vault(
        vec![prov("p1", "https://p1.test/v1", "m1", "free")],
        "p1",
    );
    let mut d = deps(&v, &t);
    d.timeout = Duration::from_secs(7);
    let res = execute_chat_with("hola", Some("p1"), None, None, 10_000_000, &d);
    assert_eq!(res.model_id, "p1");
    let cuerpo = t.cuerpo_enviado("https://p1.test/v1").expect("no se envió la petición");
    let mt = cuerpo["max_tokens"].as_u64().expect("max_tokens ausente");
    assert!(
        mt <= MAX_TOKENS_CAP as u64,
        "max_tokens sin acotar: {}",
        mt
    );
    assert_eq!(
        t.timeout_usado("https://p1.test/v1"),
        Some(Duration::from_secs(7)),
        "el timeout por proveedor no se pasó al transporte"
    );
}

#[test]
fn max_tokens_cero_usa_el_defecto() {
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, OK);
    let v = vault(
        vec![prov("p1", "https://p1.test/v1", "m1", "free")],
        "p1",
    );
    let d = deps(&v, &t);
    let _ = execute_chat_with("hola", Some("p1"), None, None, 0, &d);
    let cuerpo = t.cuerpo_enviado("https://p1.test/v1").expect("no se envió la petición");
    let mt = cuerpo["max_tokens"].as_u64().expect("max_tokens ausente");
    assert!(mt > 0, "max_tokens 0 no puede ser 0: {}", mt);
    assert!(mt <= MAX_TOKENS_CAP as u64);
}

#[test]
fn un_proveedor_sin_credencial_se_salta() {
    let t = MockTransport::new();
    t.responde("https://p2.test/v1", 200, OK);
    let mut p1 = prov("p1", "https://p1.test/v1", "m1", "paid");
    p1.api_key = String::new();
    let v = vault(vec![p1, prov("p2", "https://p2.test/v1", "m2", "free")], "p1");
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(t.veces("https://p1.test/v1"), 0);
    assert_eq!(res.model_id, "p2");
    assert!(res
        .provider_trace
        .iter()
        .any(|l| l.contains("credencial")));
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Funciones puras: redacción, recorte y validación de endpoint
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn la_redaccion_tapa_la_clave() {
    assert_eq!(
        redact_secret("Invalid API key: sk-abc123", "sk-abc123"),
        "Invalid API key: ***"
    );
    assert_eq!(
        redact_secret("Authorization: Bearer sk-abc123", "sk-abc123"),
        "Authorization: Bearer ***"
    );
    assert_eq!(redact_secret("sin secreto", ""), "sin secreto");
}

#[test]
fn truncar_noparte_por_la_mitad_un_utf8() {
    let s = "a 🦾 b 🧠 c";
    for n in 0..=s.chars().count() {
        let t = truncar(s, n);
        assert!(t.chars().count() <= n);
    }
    assert_eq!(truncar(s, 3), "a 🦾");
}

#[test]
fn validar_endpoint_rechaza_lo_peligroso() {
    assert!(validar_endpoint("https://api.minimax.io/v1/chat/completions").is_ok());
    assert!(validar_endpoint("http://127.0.0.1:8080/v1/chat/completions").is_ok());
    assert!(validar_endpoint("file:///data/local/tmp/x").is_err());
    assert!(validar_endpoint("http://169.254.169.254/latest/meta-data/").is_err());
    assert!(validar_endpoint("http://user:pass@host/v1").is_err());
    assert!(validar_endpoint("   ").is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. PROPIEDAD: el parseo NUNCA entra en panic y siempre dice algo útil
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn propiedad_el_parseo_nunca_entra_en_panic_para_ningun_corte() {
    for f in fixtures::FIXTURES {
        let cuerpo = fixtures::cuerpo(f.file);
        let cortes = cuerpo.chars().count();
        for n in 0..=cortes {
            let trozo: String = cuerpo.chars().take(n).collect();
            match parse_provider_reply(f.provider, &trozo) {
                Ok(r) => {
                    assert!(
                        !r.content.is_empty() || !r.reasoning.is_empty() || !r.tool_calls.is_empty(),
                        "{} (corte {}) devolvió una respuesta vacía",
                        f.file,
                        n
                    );
                }
                Err(e) => {
                    let msg = e.to_string();
                    assert!(
                        !msg.trim().is_empty(),
                        "{} (corte {}) devolvió un error sin mensaje",
                        f.file,
                        n
                    );
                }
            }
        }
    }
}

#[test]
fn propiedad_el_parseo_de_error_nunca_entra_en_panic() {
    for f in fixtures::FIXTURES {
        let cuerpo = fixtures::cuerpo(f.file);
        for status in [400u16, 401, 402, 404, 429, 500, 503] {
            let e = parse_provider_error(f.provider, status, &cuerpo);
            assert_eq!(e.status(), Some(status));
            assert!(!e.detail().is_empty(), "{} sin detalle", f.file);
            assert!(!e.to_string().is_empty());
        }
    }
}

#[test]
fn todos_los_fixtures_declaran_su_origen_y_no_tienen_credenciales() {
    for f in fixtures::FIXTURES {
        let v = fixtures::fichero(f.file);
        let meta = &v["_meta"];
        assert_eq!(meta["id"].as_str().unwrap_or(""), f.file.trim_end_matches(".json"));
        assert_eq!(meta["provider"].as_str().unwrap_or(""), f.provider);
        assert_eq!(meta["status"].as_u64().unwrap_or(0), f.status as u64);
        let origen = meta["origen"].as_str().unwrap_or("");
        assert!(
            origen == "REAL" || origen == "SINTETICO",
            "{}: origen desconocido '{}'",
            f.file,
            origen
        );
        assert!(
            !meta["nota"].as_str().unwrap_or("").is_empty(),
            "{}: sin nota de procedencia",
            f.file
        );
        // Ninguna credencial real en el banco.
        let texto = serde_json::to_string(&v).unwrap();
        assert!(
            !texto.contains("sk-prod-") && !texto.contains("AIza") && !texto.contains("Bearer sk-"),
            "{} parece llevar una credencial real",
            f.file
        );
    }
}

#[test]
fn el_indice_del_banco_apunta_a_ficheros_que_existen() {
    for f in fixtures::FIXTURES {
        assert!(
            fixtures::ruta(f.file).exists(),
            "falta el fixture {}",
            f.file
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Casos raros del banco, uno a uno
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn el_razonamiento_por_campo_se_separa_del_texto() {
    let cuerpo = fixtures::cuerpo("minimax-m3.1-flash__reasoning_field.json");
    let r = parse_provider_reply("minimax-m3.1-flash", &cuerpo).unwrap();
    assert_eq!(r.content, "La respuesta final es 42.");
    assert!(r.reasoning.contains("compruebo el enunciado"));
    assert_eq!(r.tokens_used, 55);
}

#[test]
fn el_razonamiento_think_se_saca_del_texto() {
    let cuerpo = fixtures::cuerpo("minimax-m3.1-flash__reasoning_think.json");
    let r = parse_provider_reply("minimax-m3.1-flash", &cuerpo).unwrap();
    assert_eq!(r.content, "La respuesta final es 42.");
    assert!(r.reasoning.to_lowercase().contains("el usuario pide"));
    assert!(!r.content.contains("<think>"));
}

#[test]
fn el_contenido_en_array_se_une() {
    let cuerpo = fixtures::cuerpo("openrouter-free__content_array.json");
    let r = parse_provider_reply("openrouter-free", &cuerpo).unwrap();
    assert!(r.content.contains("parte 1"));
    assert!(r.content.contains("parte 2"));
}

#[test]
fn las_tool_calls_nativas_se_parsean() {
    let cuerpo = fixtures::cuerpo("openrouter-free__tool_calls.json");
    let r = parse_provider_reply("openrouter-free", &cuerpo).unwrap();
    assert_eq!(r.tool_calls.len(), 1);
    assert_eq!(r.tool_calls[0].name, "leer_fichero");
    assert_eq!(r.tool_calls[0].arguments["ruta"], "/tmp/ejemplo.bin");
    assert_eq!(r.finish_reason, "tool_calls");
}

#[test]
fn unas_tool_calls_con_arguments_rotos_no_entran_en_panic() {
    let cuerpo = fixtures::cuerpo("openrouter-free__tool_calls_bad_arguments.json");
    let r = parse_provider_reply("openrouter-free", &cuerpo).unwrap();
    assert_eq!(r.tool_calls.len(), 1);
    assert_eq!(r.tool_calls[0].name, "leer_fichero");
}

#[test]
fn un_choices_vacio_es_error_con_mensaje_util() {
    let cuerpo = fixtures::cuerpo("ovhcloud-free__empty_choices.json");
    let e = parse_provider_reply("ovhcloud-free", &cuerpo).unwrap_err();
    assert_eq!(e.kind(), "parse");
    assert!(!e.to_string().is_empty());
}

#[test]
fn un_error_dentro_de_un_200_es_fallo_del_proveedor() {
    let cuerpo = fixtures::cuerpo("openai-compatible__error_in_200.json");
    let e = parse_provider_reply("openai-compatible", &cuerpo).unwrap_err();
    assert!(e.to_string().contains("Model overloaded"));
}

#[test]
fn un_texto_plano_se_acepta_tal_cual() {
    let cuerpo = fixtures::cuerpo("pollinations-free__text_plain.json");
    let r = parse_provider_reply("pollinations-free", &cuerpo).unwrap();
    assert!(r.content.starts_with("Pollinations responde en texto plano"));
}

#[test]
fn una_forma_legacy_se_acepta() {
    let cuerpo = fixtures::cuerpo("openai-compatible__legacy_text.json");
    let r = parse_provider_reply("openai-compatible", &cuerpo).unwrap();
    assert_eq!(r.content, "Respuesta en forma legacy.");
}

#[test]
fn un_429_dice_el_motivo_de_la_cuota() {
    let cuerpo = fixtures::cuerpo("groq-free__http_429.json");
    let e = parse_provider_error("groq-free", 429, &cuerpo);
    assert_eq!(e.status(), Some(429));
    assert!(e.retryable(), "un 429 debe ser reintentable");
    assert!(e.detail().contains("rate limit") || e.detail().contains("Rate limit"));
    // Y el quota_guard tiene que poder entenderlo.
    let detectada = megadaemon_core::quota_guard::parsear_error_cuota(&format!(
        "HTTP 429 (groq-free): {}",
        e.detail()
    ));
    assert!(detectada.is_some(), "el quota_guard no entendio el 429");
    let d = detectada.unwrap();
    assert_eq!(d.reset_at, Some(1795130605));
    assert_eq!(d.retry_after, Some(42));
}

// ─────────────────────────────────────────────────────────────────────────────
// Patrón 5: el `retry_after` del proveedor viaja en el error (no se ignora)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn el_retry_after_del_cuerpo_viaja_en_el_error() {
    // MEDIDO (2026-11-28): el `retry_after` se parseaba del texto **sólo**
    // en `quota_guard` (para la pausa por cuota).  El error que usa el bucle
    // de reintentos NO lo llevaba ⇒ se reintentaba antes de tiempo y el
    // proveedor volvía a contestar 429.
    let cuerpo = fixtures::cuerpo("groq-free__http_429.json");
    let e = parse_provider_error("groq-free", 429, &cuerpo);
    assert_eq!(e.retry_after(), Some(42));
}

#[test]
fn la_cabecera_retry_after_manda_sobre_el_cuerpo() {
    // La cabecera es la fuente fiable; el cuerpo es texto libre.
    let cuerpo = fixtures::cuerpo("groq-free__http_429.json");
    let e = parse_provider_error_con_retry("groq-free", 429, &cuerpo, Some(7));
    assert_eq!(e.retry_after(), Some(7));
}

#[test]
fn un_error_sin_retry_after_no_inventa() {
    let e = parse_provider_error("p1", 401, r#"{"error":{"message":"Invalid API key"}}"#);
    assert_eq!(e.retry_after(), None);
    assert!(!e.retryable());
}

#[test]
fn la_traza_dice_cuanto_pidio_esperar_el_proveedor() {
    let t = MockTransport::new();
    t.responde(
        "https://p1.test/v1",
        429,
        r#"{"error":{"message":"Rate limit reached. Please try again in 42 seconds"}}"#,
    );
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "free"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    let traza = res.provider_trace.join(" | ");
    assert!(
        traza.contains("retry_after 42s"),
        "la traza debe decir cuanto hay que esperar: {}",
        traza
    );
}

#[test]
fn un_401_no_es_reintentable() {
    let cuerpo = fixtures::cuerpo("minimax-m3.1-flash__http_401.json");
    let e = parse_provider_error("minimax-m3.1-flash", 401, &cuerpo);
    assert_eq!(e.status(), Some(401));
    assert!(!e.retryable(), "un 401 no se reintenta");
    assert!(e.detail().contains("API key invalida"));
}

#[test]
fn un_500_si_es_reintentable() {
    let cuerpo = fixtures::cuerpo("anthropic__http_500.json");
    let e = parse_provider_error("anthropic", 500, &cuerpo);
    assert!(e.retryable(), "un 500 debe ser reintentable");
}

#[test]
fn un_404_no_es_reintentable() {
    let cuerpo = fixtures::cuerpo("minimax-m3.1-flash__http_404.json");
    let e = parse_provider_error("minimax-m3.1-flash", 404, &cuerpo);
    assert!(!e.retryable(), "un 404 no se reintenta");
}

#[test]
fn un_cuerpo_html_no_entra_en_panic() {
    let cuerpo = fixtures::cuerpo("pollinations-free__html_error.json");
    let e = parse_provider_error("pollinations-free", 502, &cuerpo);
    assert_eq!(e.status(), Some(502));
    assert!(!e.to_string().is_empty());
}

#[test]
fn el_400_con_max_tokens_absurdo_lo_dice_el_proveedor() {
    // Reproducción del DEFECTO-09: el daemon debe poder distinguir un
    // "max_tokens absurdo" de un fallo del proveedor.
    let e = parse_provider_error(
        "p1",
        400,
        r#"{"error":{"message":"max_tokens must be <= 32768"}}"#,
    );
    assert_eq!(e.status(), Some(400));
    assert!(e.detail().contains("max_tokens"));
    assert!(!e.retryable(), "un 400 no se reintenta");
}

#[test]
fn la_respuesta_local_no_se_parsea_como_openai() {
    // El SEERE local tiene su PROPIA forma: no es un error de parseo, es otro
    // motor.  Este test deja claro que no se puede meter por el mismo camino.
    let cuerpo = fixtures::cuerpo("local-seere__offline.json");
    let v: Value = serde_json::from_str(&cuerpo).unwrap();
    assert!(v["answer"].is_string());
    assert!(v["tools"].is_array());
    assert!(parse_provider_reply("local-seere", &cuerpo).is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. La traza
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn la_traza_dice_quien_respondio_y_quien_se_salto() {
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 401, r#"{"error":{"message":"nope"}}"#);
    t.responde("https://p3.test/v1", 200, OK);
    let mut p2 = prov("p2", "https://p2.test/v1", "m2", "free");
    p2.api_key = String::new();
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            p2,
            prov("p3", "https://p3.test/v1", "m3", "free"),
        ],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    let traza = res.provider_trace.join(" | ");
    assert!(traza.contains("p1"), "{}", traza);
    assert!(traza.contains("p3"), "{}", traza);
    assert!(traza.contains("p2"), "el salto por credencial debe verse: {}", traza);
    assert!(traza.contains("OK"), "{}", traza);
}

#[test]
fn la_traza_va_dentro_del_json_del_resultado() {
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, OK);
    let v = vault(
        vec![prov("p1", "https://p1.test/v1", "m1", "free")],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    let json = serde_json::to_value(&res).unwrap();
    assert!(json["provider_trace"].is_array());
    let traza = json["provider_trace"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect::<Vec<_>>();
    // MEDIDO (2026-11-27): la traza es una CRONOLOGÍA de la cascada, así que
    // ya no empieza por el "OK" (ahora empieza por la línea del POST: qué
    // endpoint, qué modelo y cuántos caracteres se mandaron — sin eso no se
    // podía diagnosticar el 400 de google-ai-studio).  El contenido del
    // "tier" se sigue exigiendo (DEFECTO-18), sólo que buscandolo donde
    // está.  Los tests hermanos ya usaban `contains` para lo mismo.
    assert!(
        traza.iter().any(|t| t == "p1 => OK (tier free)"),
        "la traza dice el tier del proveedor (DEFECTO-18): {:?}",
        traza
    );
    assert!(
        traza.iter().any(|t| t.contains("=> POST https://p1.test/v1")),
        "la traza dice a qué endpoint se mandó y con qué tamaño: {:?}",
        traza
    );
    // Y el campo nuevo NO rompe la forma antigua: un JSON sin
    // `provider_trace` se sigue deserializando (serde default).
    let antiguo = r#"{"reply":"x","reasoning":"","model_id":"a","model_name":"b","vendor":"c","latency_ms":1.0,"tokens_used":0,"tools_used":[],"fallback_used":false}"#;
    let r: ChatResult = serde_json::from_str(antiguo).expect("no se pudo deserializar");
    assert!(r.provider_trace.is_empty());
}

static DAILY_BUDGET_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct BudgetGuard;
impl Drop for BudgetGuard {
    fn drop(&mut self) {
        megadaemon_core::providers::set_daily_budget_usd(None);
        megadaemon_core::providers::reset_daily_budget();
    }
}

#[test]
fn registrar_uso_y_fleet_cost_se_registran_en_cada_respuesta() {
    let _lock = DAILY_BUDGET_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let _guard = BudgetGuard;
    megadaemon_core::providers::reset_daily_budget();
    megadaemon_core::providers::set_daily_budget_usd(None);

    let ok_with_usage = r#"{"choices":[{"message":{"content":"ok"}}],"usage":{"prompt_tokens":1000,"completion_tokens":1000,"total_tokens":2000}}"#;
    let t = MockTransport::new();
    t.responde("https://p-usage.test/v1", 200, ok_with_usage);
    let v = vault(
        vec![prov("p-usage-test", "https://p-usage.test/v1", "gpt-4o-mini", "paid")],
        "p-usage-test",
    );
    let d = deps(&v, &t);
    // MEDIDO 2026-11-23: los contadores del presupuesto diario son de
    // PROCESO (globales) y `cargo test` corre los tests de este binario en
    // PARALELO: cualquier otro test que hace una llamada con exito tambien
    // los incrementa => los valores ABSOLUTOS ("1 llamada", "2000 tokens")
    // dependian del orden de ejecucion (rojo intermitente en el CI).  Se
    // comprueba el DELTA de esta llamada.
    let antes = megadaemon_core::providers::daily_budget_snapshot();
    let res = execute_chat_with("hola", Some("p-usage-test"), None, None, 128, &d);
    assert_eq!(res.model_id, "p-usage-test");
    assert_eq!(res.tokens_used, 2000);

    let snap = megadaemon_core::providers::daily_budget_snapshot();
    let llamadas = snap["calls_total"].as_u64().unwrap_or(0)
        .saturating_sub(antes["calls_total"].as_u64().unwrap_or(0));
    let tokens = snap["tokens_total"].as_u64().unwrap_or(0)
        .saturating_sub(antes["tokens_total"].as_u64().unwrap_or(0));
    assert!(
        llamadas >= 1,
        "la llamada debe registrarse en el presupuesto diario: {}",
        snap
    );
    assert!(
        tokens >= 2000,
        "la llamada debe registrar sus 2000 tokens: {}",
        snap
    );
    // El GASTO si es exacto: solo lo mueven los proveedores con precio
    // (este test y el del tope, y ambos tienen el cerrojo puesto).
    let spend = snap["spend_usd"].as_f64().unwrap();
    assert!(
        (spend - 0.00075).abs() < 1e-9,
        "gpt-4o-mini 1k in + 1k out debe costar 0.00075 USD, obtenido {}",
        spend
    );

    let estados = megadaemon_core::quota_guard::estado(Some("p-usage-test"));
    assert!(
        !estados.is_empty() && estados[0].ventanas.iter().any(|w| w.nombre == "tokens"),
        "quota_guard::registrar_uso debe haber registrado la ventana 'tokens': {:?}",
        estados
    );
}

#[test]
fn presupuesto_diario_bloquea_pago_al_superar_limite_y_permite_gratis() {
    let _lock = DAILY_BUDGET_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let _guard = BudgetGuard;
    megadaemon_core::providers::reset_daily_budget();
    megadaemon_core::providers::set_daily_budget_usd(Some(0.0005));

    let ok_with_usage = r#"{"choices":[{"message":{"content":"ok"}}],"usage":{"prompt_tokens":1000,"completion_tokens":1000,"total_tokens":2000}}"#;
    let t = MockTransport::new();
    t.responde("https://paid.test/v1", 200, ok_with_usage);
    t.responde("https://free.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p-paid", "https://paid.test/v1", "gpt-4o-mini", "paid"),
            prov("p-free", "https://free.test/v1", "m-free", "free"),
        ],
        "p-paid",
    );
    let d = deps(&v, &t);

    // 1a llamada: spend=0 < 0.0005 -> entra en p-paid, gasta 0.00075 USD (> 0.0005).
    let r1 = execute_chat_with("hola", Some("p-paid"), None, None, 128, &d);
    assert_eq!(r1.model_id, "p-paid");
    assert_eq!(t.veces("https://paid.test/v1"), 1);

    // 2a llamada: spend=0.00075 >= 0.0005 -> salta p-paid por presupuesto y cae a p-free.
    let r2 = execute_chat_with("hola", Some("p-paid"), None, None, 128, &d);
    assert_eq!(r2.model_id, "p-free");
    assert_eq!(t.veces("https://paid.test/v1"), 1, "no debe volver a llamar a p-paid");
    assert_eq!(t.veces("https://free.test/v1"), 1);
    assert!(
        r2.provider_trace.iter().any(|l| l.contains("presupuesto")),
        "traza debe incluir salto por presupuesto: {:?}",
        r2.provider_trace
    );
}


// ─────────────────────────────────────────────────────────────────────────────
// 6. Casos que faltaban (v16): default del vault, camino local, credencial
//    opcional, fallo de transporte, respuesta ilegible y TIER en la traza.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn sin_modelo_se_usa_el_default_del_vault() {
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, OK);
    let v = vault(vec![prov("p1", "https://p1.test/v1", "m1", "free")], "p1");
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", None, None, None, 128, &d);
    assert_eq!(
        res.model_id, "p1",
        "sin modelo pedido debe usar el default del vault"
    );
    assert_eq!(t.veces("https://p1.test/v1"), 1);
}

#[test]
fn el_camino_local_explicito_no_llama_a_nadie() {
    let t = MockTransport::new();
    let v = vault(vec![prov("p1", "https://p1.test/v1", "m1", "free")], "p1");
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("local-seere"), None, None, 128, &d);
    assert_eq!(res.model_id, "local-seere");
    assert!(
        t.endpoints().is_empty(),
        "el camino local NO debe tocar la red: {:?}",
        t.endpoints()
    );
}

#[test]
fn un_proveedor_con_auth_none_se_llama_sin_credencial() {
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, OK);
    let mut p = prov("p1", "https://p1.test/v1", "m1", "free");
    p.auth_type = "none".to_string();
    p.api_key = String::new();
    let v = vault(vec![p], "p1");
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(
        res.model_id, "p1",
        "auth_type=none debe poder llamarse sin credencial"
    );
    assert_eq!(t.veces("https://p1.test/v1"), 1);
}

#[test]
fn un_fallo_de_transporte_cae_al_siguiente() {
    let t = MockTransport::new();
    t.cae("https://p1.test/v1");
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(
        res.model_id, "p2",
        "un fallo de transporte debe caer al siguiente proveedor"
    );
    assert!(res.fallback_used);
    assert!(
        res.provider_trace.iter().any(|l| l.contains("p1")),
        "la traza debe mencionar el fallo de p1: {:?}",
        res.provider_trace
    );
}

#[test]
fn un_cuerpo_que_no_es_json_se_devuelve_como_texto_plano() {
    // v16: `parse_provider_reply` es DELIBERADAMENTE tolerante con los
    // endpoints que responden texto plano (los zero-key devuelven HTML o
    // texto): no se cae al siguiente proveedor, se devuelve el cuerpo tal
    // cual.  Este test fija ese comportamiento para que no cambie por
    // accidente (y para que quede escrito que NO hay fallback aqui).
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, "esto no es json");
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "paid"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert_eq!(res.model_id, "p1");
    assert_eq!(res.reply, "esto no es json");
    assert!(!res.fallback_used, "el cuerpo plano NO dispara el fallback");
    assert_eq!(
        t.veces("https://p2.test/v1"),
        0,
        "no se debe llamar al siguiente proveedor"
    );
}

#[test]
fn la_traza_dice_el_tier_del_proveedor() {
    // v16 (DEFECTO-18): el reenvío a un proveedor de PAGO tiene que ser
    // visible en la traza; si no, el usuario no sabe que se gastó dinero.
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, OK);
    let v = vault(vec![prov("p1", "https://p1.test/v1", "m1", "paid")], "p1");
    let d = deps(&v, &t);
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert!(
        res.provider_trace
            .iter()
            .any(|l| l.contains("p1") && l.contains("tier paid")),
        "la traza debe decir el tier: {:?}",
        res.provider_trace
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Patrón 5: el bucle ESPERA lo que dice el proveedor, y el proveedor ANTHROPIC
// habla SU dialecto (el catálogo lo declaraba, el código no lo speakaba)
// ─────────────────────────────────────────────────────────────────────────────

/// Las esperas que el bucle de reintentos pidió (patrón 5).
///
/// MEDIDO (2026-11-28): antes la espera era un `thread::sleep` clavado y los
/// tests usaban `backoff_ms = 0` ⇒ `espera_reintento` devolvía CERO y el
/// `retry_after` del proveedor quedaba **enmascarado**: ningún test podía
/// detectar que el bucle hubiera vuelto al backoff fijo.
static ESPERAS: std::sync::Mutex<Vec<Duration>> = std::sync::Mutex::new(Vec::new());

/// Hook de espera: **apunta** en vez de dormir.
fn registrar_espera(espera: Duration) {
    ESPERAS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push(espera);
}

/// Las esperas registradas (copia).
fn esperas() -> Vec<Duration> {
    ESPERAS.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// Ahora, en segundos Unix (para comparar con un `reset_at`).
fn ahora_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Un proveedor de Anthropic con su `auth_type` real.
fn prov_anthropic() -> ProviderEntry {
    let mut p = prov(
        "anthropic",
        "https://api.anthropic.com/v1/messages",
        "claude-sonnet-5",
        "paid",
    );
    p.auth_type = "anthropic".to_string();
    p
}

/// Respuesta de Anthropic (dialecto `content` con bloques).
fn cuerpo_anthropic_ok() -> String {
    serde_json::json!({
        "id": "msg_01",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": "PONG"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 11, "output_tokens": 3},
    })
    .to_string()
}

#[test]
fn el_bucle_espera_lo_que_dice_el_proveedor() {
    // MEDIDO (2026-11-28): el `retry_after` viaja en el error, pero la
    // espera era invisible ⇒ el "esperar lo que dice el proveedor" no lo
    // fijaba NINGÚN test.  Con el hook de espera sí.
    ESPERAS.lock().unwrap_or_else(|p| p.into_inner()).clear();
    let t = MockTransport::new();
    t.responde_n(
        "https://p1.test/v1",
        429,
        r#"{"error":{"message":"rate limit reached. Please try again in 42 seconds"}}"#,
        5,
    );
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "free"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let mut d = deps(&v, &t);
    d.retries = 1;
    // Backoff fijo de 250 ms: si el bucle lo usara, esperaría 500 ms.
    d.backoff_ms = 250;
    d.dormir = registrar_espera;
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    let esperas = esperas();
    assert_eq!(esperas.len(), 1, "hubo que esperar una vez: {:?}", esperas);
    assert_eq!(
        esperas[0],
        Duration::from_secs(42),
        "el bucle debe esperar lo que dice el proveedor (42 s), no el backoff fijo"
    );
    assert_eq!(res.model_id, "p2", "tras el 429 se pasa al siguiente");
}

#[test]
fn un_401_no_espera_nada() {
    // Un 4xx que no se reintenta tampoco se espera: dormir para nada
    // retrasa al usuario sin motivo.
    ESPERAS.lock().unwrap_or_else(|p| p.into_inner()).clear();
    let t = MockTransport::new();
    t.responde_n(
        "https://p1.test/v1",
        401,
        r#"{"error":{"message":"nope"}}"#,
        5,
    );
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p1", "https://p1.test/v1", "m1", "free"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p1",
    );
    let mut d = deps(&v, &t);
    d.retries = 2;
    d.backoff_ms = 250;
    d.dormir = registrar_espera;
    let res = execute_chat_with("hola", Some("p1"), None, None, 128, &d);
    assert!(
        esperas().is_empty(),
        "un 401 no se reintenta ⇒ no se espera: {:?}",
        esperas()
    );
    assert_eq!(res.model_id, "p2");
}

#[test]
fn el_retry_after_de_la_cabecera_llega_a_la_pausa_de_cuota() {
    // MEDIDO (2026-11-28): el `retry_after` de la CABECERA se perdía al
    // llegar al quota_guard (sólo se pasaba el TEXTO del error) ⇒ la pausa
    // se caía a un cooldown a ciegas en vez de esperar lo que pidió el
    // proveedor.
    let cuerpo = "rate limit reached";
    megadaemon_core::quota_guard::registrar_error_cuota_con_retry_after(
        "p5-cabecera",
        429,
        cuerpo,
        Some(42),
    )
    .expect("se registra el 429");
    let estado = megadaemon_core::quota_guard::estado(Some("p5-cabecera"));
    let ventana = estado
        .first()
        .and_then(|e| e.ventanas.iter().find(|v| v.nombre == "cupo"))
        .expect("hay ventana de cuota");
    let reset_at = ventana
        .reset_at
        .expect("el reset_at de la cabecera debe llegar");
    let delta = reset_at.saturating_sub(ahora_unix());
    assert!(
        (40..=45).contains(&delta),
        "la pausa debe durar lo que dijo el proveedor (42 s), no el cooldown: {} s",
        delta
    );

    // Y SIN `retry_after` estructurado no se inventa un reset_at (el
    // cooldown acotado es lo que hay).
    megadaemon_core::quota_guard::registrar_error_cuota("p5-sin-cabecera", 429, cuerpo)
        .expect("se registra el 429");
    let estado2 = megadaemon_core::quota_guard::estado(Some("p5-sin-cabecera"));
    let ventana2 = estado2
        .first()
        .and_then(|e| e.ventanas.iter().find(|v| v.nombre == "cupo"))
        .expect("hay ventana de cuota");
    assert!(
        ventana2.reset_at.is_none(),
        "sin `retry_after` no se inventa un reset_at: {:?}",
        ventana2.reset_at
    );
}

#[test]
fn la_cascada_pasa_el_retry_after_de_la_cabecera_a_la_pausa() {
    // MEDIDO (2026-11-28): el `retry_after` de la CABECERA se perdía al
    // llegar al quota_guard (sólo se pasaba el TEXTO del error) ⇒ la pausa
    // se caía a un cooldown a ciegas.  Aquí el proveedor dice "42 s" en la
    // cabecera y el cuerpo NO lo repite: la cascada tiene que arrastrarlo.
    let t = MockTransport::new();
    // El cuerpo NO menciona el tiempo de espera.
    t.responde_con_retry_after(
        "https://p1.test/v1",
        429,
        r#"{"error":{"message":"rate limit reached"}}"#,
        42,
    );
    t.responde("https://p2.test/v1", 200, OK);
    let v = vault(
        vec![
            prov("p5-cascada", "https://p1.test/v1", "m1", "free"),
            prov("p2", "https://p2.test/v1", "m2", "free"),
        ],
        "p5-cascada",
    );
    let mut d = deps(&v, &t);
    d.retries = 0;
    d.dormir = registrar_espera;
    let _ = execute_chat_with("hola", Some("p5-cascada"), None, None, 128, &d);
    let estado = megadaemon_core::quota_guard::estado(Some("p5-cascada"));
    let ventana = estado
        .first()
        .and_then(|e| e.ventanas.iter().find(|v| v.nombre == "cupo"))
        .expect("la cascada debe dejar al proveedor en pausa");
    let reset_at = ventana
        .reset_at
        .expect("el reset_at debe venir de la cabecera");
    let delta = reset_at.saturating_sub(ahora_unix());
    assert!(
        (40..=45).contains(&delta),
        "la pausa debe durar los 42 s que pidió el proveedor: {} s",
        delta
    );
}

#[test]
fn un_motor_offline_no_recibe_tools_estructuradas() {
    // Patrón 5(c): los motores **offline** no hablan tool-calling ⇒ sus
    // tools van como TEXTO en el prompt.  Mandárselas estructuradas gasta
    // contexto explicándolas sin que el motor pueda usarlas.
    let t = MockTransport::new();
    t.responde("https://p1.test/v1", 200, OK);
    let v = vault(
        vec![prov("p1", "https://p1.test/v1", "local-seere", "free")],
        "p1",
    );
    let d = deps(&v, &t);
    let tools = vec![serde_json::json!({
        "type": "function",
        "function": {
            "name": "leer",
            "description": "lee un fichero",
            "parameters": {"type": "object", "properties": {}}
        }
    })];
    let res = execute_chat_con_tools_with("hola", Some("p1"), None, None, 128, &tools, None, &d);
    let cuerpo = t
        .cuerpo_enviado("https://p1.test/v1")
        .expect("se llamó al proveedor");
    assert!(
        cuerpo.get("tools").is_none(),
        "un motor offline NO recibe tools estructuradas: {}",
        cuerpo
    );
    let system = cuerpo["messages"][0]["content"].as_str().unwrap_or("");
    assert!(
        system.contains("leer"),
        "las tools van como TEXTO en el prompt: {}",
        system
    );
    assert!(
        res.provider_trace
            .iter()
            .any(|l| l.contains("tools como TEXTO")),
        "la traza lo dice: {:?}",
        res.provider_trace
    );
}

#[test]
fn el_proveedor_anthropic_habla_su_dialecto() {
    // MEDIDO (2026-11-28): el catálogo declara `AnthropicTools` para
    // `api.anthropic.com` y Anthropic es proveedor de primera clase, pero
    // el cuerpo que se le mandaba era el de **OpenAI** (`messages` con rol
    // `system`, `tools: [{type:"function",…}]`, `tool_choice: "auto"`) ⇒
    // la API lo rechaza con HTTP 400.  El formateador decía una cosa y el
    // código otra.
    let t = MockTransport::new();
    t.responde(
        "https://api.anthropic.com/v1/messages",
        200,
        &cuerpo_anthropic_ok(),
    );
    let v = vault(vec![prov_anthropic()], "anthropic");
    let d = deps(&v, &t);
    let tools = vec![serde_json::json!({
        "type": "function",
        "function": {
            "name": "leer",
            "description": "lee un fichero",
            "parameters": {"type": "object", "properties": {"path": {"type": "string"}}}
        }
    })];
    let res = execute_chat_con_tools_with(
        "hola",
        Some("anthropic"),
        None,
        Some("eres un agente"),
        128,
        &tools,
        None,
        &d,
    );
    let cuerpo = t
        .cuerpo_enviado("https://api.anthropic.com/v1/messages")
        .expect("se llamó a anthropic");
    // El `system` va ARRIBA (en Anthropic no es un mensaje).
    assert_eq!(
        cuerpo["system"], "eres un agente",
        "el system de Anthropic va arriba: {}",
        cuerpo
    );
    // Y las tools van con el dialecto de Anthropic.
    let tool = &cuerpo["tools"][0];
    assert_eq!(tool["name"], "leer");
    assert!(
        tool.get("function").is_none(),
        "las tools NO van en el envoltorio de OpenAI: {}",
        tool
    );
    assert!(
        tool["input_schema"]["properties"]["path"].is_object(),
        "el schema va en `input_schema`: {}",
        tool
    );
    assert_eq!(
        cuerpo["tool_choice"]["type"], "auto",
        "`tool_choice` de Anthropic es un objeto: {}",
        cuerpo["tool_choice"]
    );
    // Y la respuesta se lee con el dialecto de Anthropic.
    assert_eq!(res.reply, "PONG", "la respuesta se lee de `content`");
    assert!(!res.fallback_used);
}

#[test]
fn la_respuesta_de_anthropic_se_lee_con_su_dialecto() {
    // `content` es una lista de BLOQUES: `text`, `tool_use` y `thinking`.
    let cuerpo = serde_json::json!({
        "content": [
            {"type": "thinking", "thinking": "voy a leerlo"},
            {"type": "text", "text": "Voy a leerlo."},
            {"type": "tool_use", "id": "toolu_1", "name": "leer", "input": {"path": "/tmp/x"}}
        ],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 12, "output_tokens": 7},
    })
    .to_string();
    let r = parse_anthropic_reply("anthropic", &cuerpo).expect("se parsea");
    assert_eq!(r.content, "Voy a leerlo.");
    assert_eq!(r.reasoning, "voy a leerlo");
    assert_eq!(
        r.tool_calls.len(),
        1,
        "el bloque `tool_use` es una tool call"
    );
    assert_eq!(r.tool_calls[0].name, "leer");
    assert_eq!(r.tool_calls[0].arguments["path"], "/tmp/x");
    assert_eq!(r.finish_reason, "tool_use");
    assert_eq!(r.prompt_tokens, 12);
    assert_eq!(r.completion_tokens, 7);
    assert_eq!(r.tokens_used, 19);
}

#[test]
fn un_error_de_anthropic_no_se_confunde_con_una_respuesta() {
    // La API de Anthropic devuelve los errores como `{"type":"error",…}`.
    let cuerpo = serde_json::json!({
        "type": "error",
        "error": {"type": "invalid_request_error", "message": "max_tokens is required"}
    })
    .to_string();
    let e = parse_anthropic_reply("anthropic", &cuerpo).expect_err("un error no es una respuesta");
    assert_eq!(e.kind(), "parse");
    assert!(
        e.detail().contains("max_tokens is required"),
        "el error dice qué pasó: {}",
        e.detail()
    );
}
