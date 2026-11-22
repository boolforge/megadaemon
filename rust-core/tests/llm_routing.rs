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
    execute_chat_with, parse_provider_error, parse_provider_reply, redact_secret, truncar,
    validar_endpoint, ChatResult, ProviderEntry, MAX_TOKENS_CAP,
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
fn el_razonamiento_<think>_se_saca_del_texto() {
    let cuerpo = fixtures::cuerpo("minimax-m3.1-flash__reasoning_think.json");
    let r = parse_provider_reply("minimax-m3.1-flash", &cuerpo).unwrap();
    assert_eq!(r.content, "La respuesta final es 42.");
    assert!(r.reasoning.contains("el usuario pide"));
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
    assert_eq!(json["provider_trace"][0].as_str().unwrap(), "p1 => OK");
    // Y el campo nuevo NO rompe la forma antigua: un JSON sin
    // `provider_trace` se sigue deserializando (serde default).
    let antiguo = r#"{"reply":"x","reasoning":"","model_id":"a","model_name":"b","vendor":"c","latency_ms":1.0,"tokens_used":0,"tools_used":[],"fallback_used":false}"#;
    let r: ChatResult = serde_json::from_str(antiguo).expect("no se pudo deserializar");
    assert!(r.provider_trace.is_empty());
}
