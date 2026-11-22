//! Suite de **routing** del chat de MegaDaemon (v16).
//!
//! Determinista y **SIN RED**: el vault, el transporte HTTP y el motor
//! local se inyectan, así que la cascada, el fallback, el presupuesto y el
//! parseo se prueban sin tocar la red ni el disco.
//!
//! Cobertura:
//!   * la cascada elige el proveedor pedido;
//!   * cae al siguiente si el primero falla;
//!   * el SEERE local es el ÚLTIMO recurso;
//!   * `fallback_used` es correcto (incluido el modelo pedido inexistente);
//!   * un proveedor RETIRADO no se usa (ni pedido ni de reserva);
//!   * un proveedor DESHABILITADO no se usa;
//!   * el fallback NO entra en un proveedor de PAGO;
//!   * el presupuesto bloquea la llamada cara;
//!   * reintento con backoff para 429/5xx y NO para 4xx;
//!   * la API key no aparece en ningún error que llegue al cliente;
//!   * `max_tokens` se acota;
//!   * `tools_used` refleja lo REALMENTE usado;
//!   * el parseo NUNCA hace panic con las respuestas capturadas.

mod fixtures;

use fixtures::{FIXTURES, ProviderFixture};
use megadaemon_core::providers::{
    self, ChatPolicy, ChatRequest, LlmRequest, LlmResponse, LlmTransport, LocalFallback,
    ProviderEntry, ProviderError, ProviderVault,
};
use megadaemon_core::service_bus::Budget;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// Transportes de mentira
// ─────────────────────────────────────────────────────────────────────────────

/// Respuesta programada de un transporte de mentira.
#[derive(Clone, Debug)]
enum MockReply {
    /// Respuesta HTTP con status y cuerpo.
    Http(u16, String),
    /// Fallo de transporte.
    Transport(String),
    /// Timeout.
    Timeout,
}

/// Transporte de mentira: responde por **endpoint** y registra las llamadas.
#[derive(Debug, Default)]
struct MockTransport {
    routes: HashMap<String, Vec<MockReply>>,
    calls: Mutex<Vec<(String, String)>>,
    counters: Mutex<HashMap<String, usize>>,
}

impl MockTransport {
    fn new() -> MockTransport {
        MockTransport::default()
    }

    /// Programa las respuestas de un endpoint (se van gastando en orden; la
    /// última se repite).
    fn route(mut self, endpoint: &str, replies: Vec<MockReply>) -> MockTransport {
        self.routes.insert(endpoint.to_string(), replies);
        self
    }

    fn ok(self, endpoint: &str, content: &str) -> MockTransport {
        let body = json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 1764000000,
            "model": "test",
            "choices": [{"index":0,"message":{"role":"assistant","content":content},"finish_reason":"stop"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        })
        .to_string();
        self.route(endpoint, vec![MockReply::Http(200, body)])
    }

    fn http(self, endpoint: &str, status: u16, body: &str) -> MockTransport {
        self.route(endpoint, vec![MockReply::Http(status, body.to_string())])
    }

    /// Endpoints que se han llamado (en orden).
    fn called(&self) -> Vec<String> {
        self.calls
            .lock()
            .map(|c| c.iter().map(|(e, _)| e.clone()).collect())
            .unwrap_or_default()
    }

    /// Nº de llamadas.
    fn call_count(&self) -> usize {
        self.calls.lock().map(|c| c.len()).unwrap_or(0)
    }
}

impl LlmTransport for MockTransport {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn post(&self, req: &LlmRequest, _timeout: Duration) -> Result<LlmResponse, ProviderError> {
        if let Ok(mut c) = self.calls.lock() {
            c.push((req.endpoint.clone(), req.model.clone()));
        }
        let replies = match self.routes.get(&req.endpoint) {
            Some(r) => r.clone(),
            None => {
                return Err(ProviderError::Transport {
                    provider: req.model.clone(),
                    detail: format!("sin ruta para {}", req.endpoint),
                })
            }
        };
        let n = {
            let mut counters = self.counters.lock().unwrap_or_else(|p| p.into_inner());
            let e = counters.entry(req.endpoint.clone()).or_insert(0);
            let cur = *e;
            *e += 1;
            cur
        };
        let reply = replies
            .get(n)
            .or_else(|| replies.last())
            .cloned()
            .unwrap_or(MockReply::Transport("sin respuestas".into()));
        match reply {
            MockReply::Http(status, body) => Ok(LlmResponse { status, body }),
            MockReply::Transport(d) => Err(ProviderError::Transport {
                provider: req.model.clone(),
                detail: d,
            }),
            MockReply::Timeout => Err(ProviderError::Timeout {
                provider: req.model.clone(),
                detail: "timeout de mentira".into(),
            }),
        }
    }
}

/// Motor local de mentira (no toca el LLM local).
#[derive(Debug, Default)]
struct StubLocal {
    answer: Mutex<String>,
    tools: Mutex<Vec<Value>>,
}

impl StubLocal {
    fn new(answer: &str) -> StubLocal {
        StubLocal {
            answer: Mutex::new(answer.to_string()),
            tools: Mutex::new(Vec::new()),
        }
    }

    fn with_tools(answer: &str, tools: Vec<Value>) -> StubLocal {
        StubLocal {
            answer: Mutex::new(answer.to_string()),
            tools: Mutex::new(tools),
        }
    }
}

impl LocalFallback for StubLocal {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn run(&self, _prompt: &str, _max_turns: usize) -> Value {
        json!({
            "answer": self.answer.lock().map(|a| a.clone()).unwrap_or_default(),
            "tools": self.tools.lock().map(|t| t.clone()).unwrap_or_default(),
            "tokens": 7,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Boveda sintetica
// ─────────────────────────────────────────────────────────────────────────────

fn provider(
    id: &str,
    endpoint: &str,
    tier: &str,
    key: &str,
    enabled: bool,
) -> ProviderEntry {
    ProviderEntry {
        id: id.to_string(),
        name: format!("Provider {}", id),
        vendor: "Test".to_string(),
        endpoint: endpoint.to_string(),
        model: format!("model-{}", id),
        api_key: key.to_string(),
        auth_type: if key.is_empty() { "none".into() } else { "bearer".into() },
        tier: tier.to_string(),
        free_signup_url: String::new(),
        enabled,
        supports_reasoning: false,
    }
}

const EP_A: &str = "https://a.test/v1/chat/completions";
const EP_B: &str = "https://b.test/v1/chat/completions";
const EP_PAID: &str = "https://paid.test/v1/chat/completions";

fn vault_free_a_then_b() -> ProviderVault {
    ProviderVault {
        default_model: "free-a".to_string(),
        providers: vec![
            provider("free-a", EP_A, "free", "key-a", true),
            provider("free-b", EP_B, "free", "key-b", true),
        ],
    }
}

fn policy_test() -> ChatPolicy {
    ChatPolicy::for_tests(Budget::new(1.0, 10.0))
}

fn run(
    vault: &ProviderVault,
    transport: &dyn LlmTransport,
    local: &dyn LocalFallback,
    policy: &ChatPolicy,
    model: &str,
    prompt: &str,
) -> providers::ChatResult {
    let req = ChatRequest::new(prompt).with_model(model);
    providers::execute_chat_in(vault, transport, local, policy, &req)
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. La cascada
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn la_cascada_elige_el_proveedor_pedido() {
    let vault = vault_free_a_then_b();
    let t = MockTransport::new().ok(EP_A, "A_OK").ok(EP_B, "B_OK");
    let res = run(&vault, &t, &StubLocal::new("LOCAL"), &policy_test(), "free-a", "hola");
    assert_eq!(res.model_id, "free-a");
    assert_eq!(res.reply, "A_OK");
    assert!(!res.fallback_used, "no hay fallback: el proveedor pedido respondio");
    assert_eq!(t.call_count(), 1, "no se debe llamar a mas de un proveedor");
    assert_eq!(t.called(), vec![EP_A.to_string()]);
}

#[test]
fn cae_al_siguiente_si_el_primero_falla() {
    let vault = vault_free_a_then_b();
    let t = MockTransport::new()
        .http(EP_A, 500, "{\"error\":{\"message\":\"boom\"}}")
        .ok(EP_B, "B_OK");
    let res = run(&vault, &t, &StubLocal::new("LOCAL"), &policy_test(), "free-a", "hola");
    assert_eq!(res.model_id, "free-b");
    assert_eq!(res.reply, "B_OK");
    assert!(res.fallback_used, "el proveedor pedido fallo: hay fallback");
    assert_eq!(t.call_count(), 2);
}

#[test]
fn seere_local_es_el_ultimo_recurso() {
    let vault = vault_free_a_then_b();
    let t = MockTransport::new()
        .http(EP_A, 500, "{\"error\":{\"message\":\"boom\"}}")
        .http(EP_B, 503, "{\"error\":{\"message\":\"boom\"}}");
    let local = StubLocal::new("LOCAL_ANSWER");
    let res = run(&vault, &t, &local, &policy_test(), "free-a", "hola");
    assert_eq!(res.model_id, "local-seere");
    assert!(res.reply.contains("LOCAL_ANSWER"));
    assert!(res.fallback_used);
    // La respuesta local avisa de los errores cloud.
    assert!(res.reply.contains("Fallback offline activo"));
}

#[test]
fn el_seere_local_no_se_usa_si_la_nube_responde() {
    let vault = vault_free_a_then_b();
    let t = MockTransport::new().ok(EP_A, "A_OK");
    let local = StubLocal::new("LOCAL_ANSWER");
    let res = run(&vault, &t, &local, &policy_test(), "free-a", "hola");
    assert_eq!(res.model_id, "free-a");
    assert!(!res.reply.contains("LOCAL_ANSWER"));
}

#[test]
fn fallback_used_es_cieto_cuando_el_modelo_pedido_no_existe() {
    // v16: antes, un id desconocido caia al primer proveedor de la lista con
    // `fallback_used = false` (mentira: no se uso lo que se pidio).
    let vault = vault_free_a_then_b();
    let t = MockTransport::new().ok(EP_A, "A_OK").ok(EP_B, "B_OK");
    let res = run(
        &vault,
        &t,
        &StubLocal::new("LOCAL"),
        &policy_test(),
        "no-existe-este-modelo",
        "hola",
    );
    assert_eq!(res.model_id, "free-a");
    assert!(
        res.fallback_used,
        "el proveedor pedido no existia: hay que avisar con fallback_used"
    );
    assert!(
        res.reply.contains("no esta en la boveda"),
        "la respuesta debe avisar del modelo desconocido: {}",
        res.reply
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Seguridad del fallback: retirado / deshabilitado / de pago
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn un_proveedor_retirado_no_se_usa_ni_pedido() {
    let mut vault = vault_free_a_then_b();
    vault.providers.push(provider(
        "minimax-m2.7",
        "https://retirado.test/v1/chat/completions",
        "paid",
        "key-retirada",
        true,
    ));
    let t = MockTransport::new()
        .ok(EP_A, "A_OK")
        .ok("https://retirado.test/v1/chat/completions", "M2_7_OK");
    let res = run(
        &vault,
        &t,
        &StubLocal::new("LOCAL"),
        &policy_test(),
        "minimax-m2.7",
        "hola",
    );
    assert!(
        res.reply.contains("MODELO RETIRADO"),
        "debe avisar de que el modelo esta retirado: {}",
        res.reply
    );
    assert_eq!(res.model_id, "minimax-m2.7");
    assert_eq!(t.call_count(), 0, "un proveedor retirado NO se llama: se avisa");
}

#[test]
fn un_proveedor_retirado_no_entra_en_la_cascada() {
    // v16: la comprobacion de "retirado" solo se hacia sobre el modelo
    // PEDIDO: un proveedor retirado de la boveda entraba en la reserva.
    let mut vault = vault_free_a_then_b();
    vault.providers.push(provider(
        "minimax-m2.7",
        "https://retirado.test/v1/chat/completions",
        "free",
        "key-retirada",
        true,
    ));
    let t = MockTransport::new()
        .http(EP_A, 500, "{\"error\":{\"message\":\"boom\"}}")
        .ok(EP_B, "B_OK")
        .ok("https://retirado.test/v1/chat/completions", "M2_7_OK");
    let res = run(&vault, &t, &StubLocal::new("LOCAL"), &policy_test(), "free-a", "hola");
    assert_eq!(res.model_id, "free-b");
    assert!(
        !t.called()
            .iter()
            .any(|e| e == "https://retirado.test/v1/chat/completions"),
        "un proveedor retirado no puede atender la llamada"
    );
}

#[test]
fn un_proveedor_deshabilitado_no_se_usa() {
    let vault = ProviderVault {
        default_model: "free-a".to_string(),
        providers: vec![
            provider("free-a", EP_A, "free", "key-a", false), // deshabilitado
            provider("free-b", EP_B, "free", "key-b", true),
        ],
    };
    let t = MockTransport::new().ok(EP_A, "A_OK").ok(EP_B, "B_OK");
    let res = run(&vault, &t, &StubLocal::new("LOCAL"), &policy_test(), "free-a", "hola");
    assert_ne!(res.model_id, "free-a", "un proveedor deshabilitado no se usa");
    assert!(
        !t.called().iter().any(|e| e == EP_A),
        "no se debe llamar a un proveedor deshabilitado"
    );
    assert!(
        res.reply.contains("DESHABILITADO"),
        "la respuesta debe avisar: {}",
        res.reply
    );
}

#[test]
fn el_fallback_no_gasta_dinero_en_un_proveedor_de_pago() {
    let mut vault = vault_free_a_then_b();
    vault.providers.push(provider("minimax-m3.1-flash", EP_PAID, "paid", "key-paid", true));
    let t = MockTransport::new()
        .http(EP_A, 500, "{\"error\":{\"message\":\"boom\"}}")
        .ok(EP_B, "B_OK")
        .ok(EP_PAID, "PAID_OK");
    let res = run(&vault, &t, &StubLocal::new("LOCAL"), &policy_test(), "free-a", "hola");
    assert_eq!(res.model_id, "free-b");
    assert!(
        !t.called().iter().any(|e| e == EP_PAID),
        "el fallback no puede gastar dinero sin que se pida"
    );
    assert!(
        res.reply.contains("de pago"),
        "la respuesta debe avisar del proveedor de pago descartado: {}",
        res.reply
    );
}

#[test]
fn el_proveedor_de_pago_pedido_explicitamente_si_se_usa() {
    let mut vault = vault_free_a_then_b();
    vault.providers.push(provider("minimax-m3.1-flash", EP_PAID, "paid", "key-paid", true));
    let t = MockTransport::new().ok(EP_PAID, "PAID_OK");
    let res = run(
        &vault,
        &t,
        &StubLocal::new("LOCAL"),
        &policy_test(),
        "minimax-m3.1-flash",
        "hola",
    );
    assert_eq!(res.model_id, "minimax-m3.1-flash");
    assert_eq!(res.reply, "PAID_OK");
    assert!(!res.fallback_used);
}

#[test]
fn el_presupuesto_bloquea_la_llamada_cara() {
    let mut vault = vault_free_a_then_b();
    vault.providers.push(provider("minimax-m3.1-flash", EP_PAID, "paid", "key-paid", true));
    let t = MockTransport::new()
        .http(EP_A, 500, "{\"error\":{\"message\":\"boom\"}}")
        .ok(EP_B, "B_OK")
        .ok(EP_PAID, "PAID_OK");
    // Presupuesto de 0 USD: ninguna llamada de pago puede pasar.
    let policy = ChatPolicy::for_tests(Budget::new(0.0, 0.0));
    let res = run(&vault, &t, &StubLocal::new("LOCAL"), &policy, "free-a", "hola");
    assert!(
        !t.called().iter().any(|e| e == EP_PAID),
        "el presupuesto debe bloquear la llamada cara"
    );
    assert!(res.reply.contains("fuera de presupuesto"));
}

#[test]
fn el_presupuesto_no_bloquea_al_proveedor_pedido() {
    let mut vault = vault_free_a_then_b();
    vault.providers.push(provider("minimax-m3.1-flash", EP_PAID, "paid", "key-paid", true));
    let t = MockTransport::new().ok(EP_PAID, "PAID_OK");
    let policy = ChatPolicy::for_tests(Budget::new(0.0, 0.0));
    let res = run(
        &vault,
        &t,
        &StubLocal::new("LOCAL"),
        &policy,
        "minimax-m3.1-flash",
        "hola",
    );
    assert_eq!(res.model_id, "minimax-m3.1-flash");
    assert!(!res.fallback_used);
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Reintentos
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn reintenta_el_429_y_acaba_respondiendo() {
    let vault = vault_free_a_then_b();
    let t = MockTransport::new().route(
        EP_A,
        vec![
            MockReply::Http(429, "{\"error\":{\"message\":\"rate limit\"}}".to_string()),
            MockReply::Http(200, ok_body("A_OK")),
        ],
    );
    let res = run(&vault, &t, &StubLocal::new("LOCAL"), &policy_test(), "free-a", "hola");
    assert_eq!(res.reply, "A_OK");
    assert_eq!(t.call_count(), 2, "un 429 se reintenta");
}

#[test]
fn no_reintenta_un_401() {
    let vault = vault_free_a_then_b();
    let t = MockTransport::new().http(
        EP_A,
        401,
        "{\"error\":{\"message\":\"API key not valid\"}}",
    );
    let res = run(&vault, &t, &StubLocal::new("LOCAL"), &policy_test(), "free-a", "hola");
    assert_ne!(res.reply, "A_OK");
    assert_eq!(
        t.call_count(),
        1,
        "un 401 NO se reintenta (la key sigue siendo mala)"
    );
}

#[test]
fn no_reintenta_un_404() {
    let vault = vault_free_a_then_b();
    let t = MockTransport::new().http(EP_A, 404, "{\"error\":{\"message\":\"no such model\"}}");
    let _ = run(&vault, &t, &StubLocal::new("LOCAL"), &policy_test(), "free-a", "hola");
    assert_eq!(t.call_count(), 1, "un 404 NO se reintenta");
}

#[test]
fn reintenta_el_500() {
    let vault = vault_free_a_then_b();
    let t = MockTransport::new().route(
        EP_A,
        vec![
            MockReply::Http(500, "{\"error\":{\"message\":\"boom\"}}".to_string()),
            MockReply::Http(200, ok_body("A_OK")),
        ],
    );
    let res = run(&vault, &t, &StubLocal::new("LOCAL"), &policy_test(), "free-a", "hola");
    assert_eq!(res.reply, "A_OK");
    assert_eq!(t.call_count(), 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Credenciales
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn la_api_key_no_aparece_en_la_respuesta_al_cliente() {
    // Un proveedor que ecoa la key en el cuerpo del error (pasa en algunos
    // gateways).  La key NUNCA debe llegar al cliente.
    let secret = "sk-SECRET-1234567890";
    let vault = ProviderVault {
        default_model: "free-a".to_string(),
        providers: vec![
            provider("free-a", EP_A, "free", secret, true),
            provider("free-b", EP_B, "free", "key-b", true),
        ],
    };
    let t = MockTransport::new()
        .http(
            EP_A,
            401,
            &format!("{{\"error\":{{\"message\":\"Invalid API key: {}\"}}", secret),
        )
        .http(EP_B, 401, &format!("{{\"error\":{{\"message\":\"Invalid API key: {}\"}}", secret));
    let res = run(&vault, &t, &StubLocal::new("LOCAL"), &policy_test(), "free-a", "hola");
    assert!(
        !res.reply.contains(secret),
        "la API key se filtró al cliente: {}",
        res.reply
    );
    assert!(!format!("{:?}", res).contains(secret));
}

#[test]
fn redact_secret_tapa_la_key_y_su_prefijo() {
    let s = "sk-abc123";
    assert_eq!(providers::redact_secret("key=sk-abc123 fin", s), "key=*** fin");
    assert_eq!(
        providers::redact_secret("Bearer sk-abc123", s),
        "Bearer ***"
    );
    assert_eq!(providers::redact_secret("nada", s), "nada");
    assert_eq!(providers::redact_secret("x", ""), "x");
}

#[test]
fn list_providers_redacted_no_devuelve_la_key() {
    // La boveda real puede no existir: el listado debe funcionar igual y
    // nunca devolver una key en claro.
    let v = providers::list_providers_redacted();
    let txt = serde_json::to_string(&v).unwrap_or_default();
    assert!(!txt.contains("sk-"), "no debe aparecer ninguna key en claro");
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Tokens y tools
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn max_tokens_se_acota_al_tope() {
    let vault = vault_free_a_then_b();
    let t = MockTransport::new().ok(EP_A, "A_OK");
    let req = ChatRequest::new("hola")
        .with_model("free-a")
        .with_max_tokens(10_000_000);
    let res = providers::execute_chat_in(
        &vault,
        &t,
        &StubLocal::new("LOCAL"),
        &policy_test(),
        &req,
    );
    assert_eq!(res.model_id, "free-a");
    // El tope del policy se aplica antes de salir a la red.
    let p = ChatPolicy::default();
    assert_eq!(p.clamp_tokens(10_000_000), providers::MAX_TOKENS_CAP);
    assert_eq!(p.clamp_tokens(0), providers::DEFAULT_MAX_TOKENS);
    assert_eq!(p.clamp_tokens(500), 500);
}

#[test]
fn tools_used_refleja_lo_realmente_usado() {
    let local = StubLocal::with_tools(
        "RESPUESTA LOCAL",
        vec![json!({"tool": "VIEW_FILE", "arg": "a.rs"})],
    );
    let vault = vault_free_a_then_b();
    let t = MockTransport::new()
        .http(EP_A, 500, "{\"error\":{\"message\":\"boom\"}}")
        .http(EP_B, 500, "{\"error\":{\"message\":\"boom\"}}");
    let res = run(&vault, &t, &local, &policy_test(), "free-a", "hola");
    assert_eq!(res.model_id, "local-seere");
    assert!(
        res.tools_used.iter().any(|x| x.contains("VIEW_FILE")),
        "la tool que se ejecutó debe aparecer en tools_used: {:?}",
        res.tools_used
    );
}

#[test]
fn tools_used_no_inventa_tools_que_no_se_ejecutaron() {
    let vault = vault_free_a_then_b();
    let t = MockTransport::new().ok(EP_A, "A_OK");
    let res = run(&vault, &t, &StubLocal::new("LOCAL"), &policy_test(), "free-a", "hola");
    assert!(
        res.tools_used.is_empty(),
        "sin tools ejecutadas, tools_used debe estar vacio: {:?}",
        res.tools_used
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Parseo: nunca panic, siempre algo o un error util
// ─────────────────────────────────────────────────────────────────────────────

fn ok_body(content: &str) -> String {
    json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "created": 1764000000,
        "model": "test",
        "choices": [{"index":0,"message":{"role":"assistant","content":content},"finish_reason":"stop"}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
    })
    .to_string()
}

#[test]
fn el_parseo_de_las_respuestas_capturadas_nunca_pania() {
    for f in FIXTURES {
        let r = providers::parse_provider_body(f.provider, f.status, f.body);
        match r {
            Ok(p) => {
                assert!(
                    !p.content.is_empty() || !p.reasoning.is_empty() || !p.tool_calls.is_empty(),
                    "fixture {}: respuesta vacia sin error",
                    f.file
                );
            }
            Err(e) => {
                assert!(
                    !e.to_string().is_empty(),
                    "fixture {}: error sin mensaje",
                    f.file
                );
                assert!(
                    !matches!(e, ProviderError::Parse { detail, .. } if detail.is_empty()),
                    "fixture {}: error de parseo sin detalle",
                    f.file
                );
            }
        }
    }
}

#[test]
fn el_parseo_separa_el_reasoning_del_texto() {
    let f = fixtures::by_file("minimax-m3.1-flash__reasoning_think").unwrap();
    let p = providers::parse_provider_body(f.provider, f.status, f.body).unwrap();
    assert_eq!(p.content, "MINIMAX_OK");
    assert!(p.reasoning.contains("responder MINIMAX_OK"));

    let f2 = fixtures::by_file("minimax-m3.1-flash__reasoning_field").unwrap();
    let p2 = providers::parse_provider_body(f2.provider, f2.status, f2.body).unwrap();
    assert_eq!(p2.content, "La respuesta es 42.");
    assert!(p2.reasoning.contains("6*7"));
}

#[test]
fn el_parseo_lee_las_tool_calls_nativas() {
    let f = fixtures::by_file("openrouter-free__tool_calls").unwrap();
    let p = providers::parse_provider_body(f.provider, f.status, f.body).unwrap();
    assert_eq!(p.tool_calls.len(), 1);
    assert_eq!(p.tool_calls[0].name, "view_file");
    assert_eq!(p.tool_calls[0].arguments["path"], "src/main.rs");
}

#[test]
fn el_parseo_acepta_content_como_array() {
    let f = fixtures::by_file("openrouter-free__content_array").unwrap();
    let p = providers::parse_provider_body(f.provider, f.status, f.body).unwrap();
    assert!(p.content.contains("parte uno") && p.content.contains("parte dos"));
}

#[test]
fn un_json_truncado_es_un_error_util_y_no_una_respuesta_basura() {
    let f = fixtures::by_file("zai-glm-free__truncated").unwrap();
    let e = providers::parse_provider_body(f.provider, f.status, f.body).unwrap_err();
    match e {
        ProviderError::Parse { detail, .. } => {
            assert!(detail.contains("truncado"), "detalle util: {}", detail);
        }
        other => panic!("se esperaba un error de parseo, {:?}", other),
    }
}

#[test]
fn un_json_sin_choices_es_un_error_util() {
    let f = fixtures::by_file("ovhcloud-free__empty_choices").unwrap();
    let e = providers::parse_provider_body(f.provider, f.status, f.body).unwrap_err();
    assert_eq!(e.kind(), "parse");
    assert!(e.to_string().contains("choices"));
}

#[test]
fn un_http_401_diagnostica_la_key() {
    let f = fixtures::by_file("google-ai-studio__http_401").unwrap();
    let e = providers::parse_provider_body(f.provider, f.status, f.body).unwrap_err();
    assert_eq!(e.status(), Some(401));
    assert!(e.to_string().contains("API key invalida"));
}

#[test]
fn un_http_402_diagnostica_el_saldo() {
    let f = fixtures::by_file("minimax-m3.1-flash__http_402").unwrap();
    let e = providers::parse_provider_body(f.provider, f.status, f.body).unwrap_err();
    assert_eq!(e.status(), Some(402));
    assert!(e.to_string().contains("sin saldo"));
}

#[test]
fn un_http_404_diagnostica_el_modelo() {
    let f = fixtures::by_file("minimax-m3.1-flash__http_404").unwrap();
    let e = providers::parse_provider_body(f.provider, f.status, f.body).unwrap_err();
    assert_eq!(e.status(), Some(404));
    assert!(e.to_string().contains("RETIRADO"));
}

#[test]
fn solo_se_reintentan_429_y_5xx() {
    let reintentable = |s: u16| {
        ProviderError::Http {
            provider: "p".into(),
            status: s,
            detail: String::new(),
        }
        .retryable()
    };
    assert!(reintentable(429));
    assert!(reintentable(500));
    assert!(reintentable(503));
    assert!(!reintentable(400));
    assert!(!reintentable(401));
    assert!(!reintentable(403));
    assert!(!reintentable(404));
    assert!(!reintentable(422));
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Propiedad: N respuestas capturadas (+ mutaciones) nunca panican
// ─────────────────────────────────────────────────────────────────────────────

/// LCG determinista (sin dependencias externas): el test es reproducible.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 11
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

#[test]
fn propiedad_el_parseo_nunca_pania_con_ninguna_respuesta_mutada() {
    let mut rng = Lcg(0xC0FFEE);
    let mut checked = 0usize;
    for f in FIXTURES {
        // El fixture tal cual.
        let _ = providers::parse_provider_body(f.provider, f.status, f.body);
        checked += 1;

        // Mutaciones deterministas: truncar, cortar por bytes, cambiar
        // status, insertar basura multibyte.
        let body = f.body;
        let mut variants: Vec<(u16, String)> = Vec::new();
        variants.push((f.status, body.to_string()));
        for cut in 1..=8 {
            if body.len() > cut {
                let end = body.len() - cut;
                let end = (0..=end)
                    .rev()
                    .find(|i| body.is_char_boundary(*i))
                    .unwrap_or(0);
                variants.push((f.status, body[..end].to_string()));
            }
        }
        for _ in 0..12 {
            let cut = rng.below(body.len().max(1));
            let cut = (0..=cut)
                .rev()
                .find(|i| body.is_char_boundary(*i))
                .unwrap_or(0);
            variants.push((f.status, body[..cut].to_string()));
            let st = [200u16, 201, 204, 400, 401, 402, 404, 429, 500, 503]
                [rng.below(10)];
            variants.push((st, body.to_string()));
        }
        // Texto raro: HTML, JSON con NaN, emoji, bytes UTF-8 invalidos via
        // replacement chars, cadena vacia.
        variants.push((200, String::new()));
        variants.push((200, "   ".to_string()));
        variants.push((200, "😀 ñ é".to_string()));
        variants.push((200, "{\"choices\":[{\"message\":{\"content\":null}}]}".into()));
        variants.push((200, "{\"choices\":null}".into()));
        variants.push((200, "null".into()));
        variants.push((200, "[]".into()));
        variants.push((200, "{\"choices\":[{\"message\":{\"content\":123}}]}".into()));

        for (status, b) in variants {
            // Lo importante: NO debe entrar en panic.
            let r = providers::parse_provider_body(f.provider, status, &b);
            match r {
                Ok(p) => {
                    assert!(
                        !p.content.is_empty()
                            || !p.reasoning.is_empty()
                            || !p.tool_calls.is_empty(),
                        "respuesta vacia sin error en {}",
                        f.file
                    );
                }
                Err(e) => {
                    assert!(!e.to_string().is_empty(), "error vacio en {}", f.file);
                }
            }
            checked += 1;
        }
    }
    assert!(checked > 500, "la property test debe cubrir muchos casos");
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Presupuesto del bus: concurrencia
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn el_presupuesto_del_bus_no_se_pasa_con_llamadas_concurrentes() {
    use megadaemon_core::service_bus::{Budget, ServiceEntry};
    // v16: `allows()` + `spend()` era un check-then-act: N hilos veian el
    // mismo "queda" y los N gastaban.  Con la reserva atomica, solo pasan
    // los que caben.
    let budget = Arc::new(Mutex::new(Budget::new(0.01, 1.0)));
    let reserved = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let b = budget.clone();
        let r = reserved.clone();
        handles.push(std::thread::spawn(move || {
            let mut g = b.lock().unwrap_or_else(|p| p.into_inner());
            if g.try_reserve(0.005).is_some() {
                r.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    assert_eq!(
        reserved.load(Ordering::SeqCst),
        2,
        "con un tope de 0.01 solo caben 2 reservas de 0.005"
    );
    let g = budget.lock().unwrap();
    assert!(g.spent_tick_usd <= 0.01 + 1e-9, "el tope no puede pasarse");
    let _ = ServiceEntry::new("x", megadaemon_core::service_bus::ServiceKind::Model);
}

#[test]
fn el_bus_rechaza_el_proveedor_caro_fuera_de_presupuesto() {
    use megadaemon_core::service_bus::{
        CostEstimate, SelectionPolicy, ServiceBus, ServiceEntry, ServiceKind, TaskType,
    };
    let mut caro = ServiceEntry::new("caro", ServiceKind::Model);
    caro.cost_estimate = CostEstimate::per_token(0.01, 0.05);
    caro.enabled = true;
    let mut gratis = ServiceEntry::new("gratis", ServiceKind::Model);
    gratis.cost_estimate = CostEstimate::free();
    gratis.enabled = true;

    let mut bus = ServiceBus::new(vec![caro, gratis]);
    bus.set_budget(Budget::new(0.0, 0.0));
    let sel = bus.select_with(SelectionPolicy::Cheapest, TaskType::Inference);
    assert_eq!(sel.chosen.as_deref(), Some("gratis"));
    assert!(
        sel.rejected.iter().any(|(id, why)| id == "caro"
            && why.contains("presupuesto")),
        "el proveedor caro debe quedar fuera por presupuesto: {:?}",
        sel.rejected
    );
}

#[test]
fn una_llamada_fallida_no_cobra_el_presupuesto() {
    use megadaemon_core::service_bus::{
        CostEstimate, ServiceBus, ServiceEntry, ServiceKind, TaskType,
    };
    let mut caro = ServiceEntry::new("caro", ServiceKind::Model);
    caro.cost_estimate = CostEstimate::per_token(0.001, 0.01);
    caro.enabled = true;
    let mut bus = ServiceBus::new(vec![caro]);
    bus.set_budget(Budget::new(1.0, 1.0));
    let mut call = |_e: &ServiceEntry| Err::<String, String>("fallo".to_string());
    let out = bus.route(TaskType::Inference, &mut call);
    assert!(!out.ok);
    assert_eq!(out.spent_usd, 0.0, "una llamada fallida no se cobra");
    let _ = ServiceKind::Model;
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Utilidades
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn truncate_utf8_noparte_por_bosque_utf8() {
    let s = "contenido de prueba, con acentos: ñ é 😀";
    for n in 0..s.len() + 5 {
        let t = providers::truncate_utf8(s, n);
        assert!(t.len() <= s.len());
        assert!(s.starts_with(&t) || s.contains(&t));
    }
    assert_eq!(providers::truncate_utf8(s, s.len()), s);
}

#[test]
fn validate_endpoint_rechaza_ssrf_y_basura() {
    assert!(providers::validate_endpoint("http://169.254.169.254/latest/meta-data/").is_err());
    assert!(providers::validate_endpoint("http://localhost:8080/x").is_err());
    assert!(providers::validate_endpoint("ftp://example.com/x").is_err());
    assert!(providers::validate_endpoint("").is_err());
    assert!(providers::validate_endpoint("no-es-url").is_err());
    assert!(providers::validate_endpoint("https://api.minimax.io/v1/chat/completions").is_ok());
}

#[test]
fn validate_model_rechaza_modelos_retirados() {
    assert!(providers::validate_model("").is_err());
    assert!(providers::validate_model("ok-model").is_ok());
    assert!(megadaemon_core::provider_catalog::is_retired("minimax-m2.7").is_some());
    assert!(megadaemon_core::provider_catalog::is_retired("minimax-m2.7-fast").is_some());
    assert!(megadaemon_core::provider_catalog::is_retired("github-models").is_some());
    assert!(megadaemon_core::provider_catalog::is_retired("minimax-m3.1-flash").is_none());
}

#[test]
fn el_fixture_del_seere_local_es_real_y_sin_credenciales() {
    let f: &ProviderFixture = fixtures::by_file("local-seere__offline").unwrap();
    assert!(f.is_real());
    assert!(!f.body.contains("sk-"));
    assert!(!f.body.contains("Bearer"));
}
