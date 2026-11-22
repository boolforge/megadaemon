//! Utilidades compartidas por las suites de chat/LLM (v16).
//!
//! * `MockTransport`: un [`LlmTransport`] de mentira ⇒ la cascada se
//!   testea **sin red** y **sin credenciales**.
//! * constructors de vault de prueba.

use megadaemon_core::providers::{
    ChatDeps, LlmRequest, LlmResponse, LlmTransport, ProviderEntry, ProviderError, ProviderVault,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Transporte de mentira: cada endpoint tiene su cola de respuestas.
#[derive(Clone)]
pub struct MockTransport {
    /// Endpoint → cola de respuestas `(status, cuerpo)`.
    por_endpoint: Arc<Mutex<HashMap<String, Vec<(u16, String)>>>>,
    /// Peticiones registradas: `(endpoint, cuerpo, timeout)`.
    llamadas: Arc<Mutex<Vec<(String, Value, Duration)>>>,
    /// Endpoints que "se caen" (error de transporte).
    caidas: Arc<Mutex<Vec<String>>>,
}

impl MockTransport {
    pub fn new() -> Self {
        MockTransport {
            por_endpoint: Arc::new(Mutex::new(HashMap::new())),
            llamadas: Arc::new(Mutex::new(Vec::new())),
            caidas: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Programa UNA respuesta para un endpoint.
    pub fn responde(&self, endpoint: &str, status: u16, cuerpo: &str) {
        self.responde_n(endpoint, status, cuerpo, 1);
    }

    /// Programa N respuestas IGUALES para un endpoint (se van gastando).
    pub fn responde_n(&self, endpoint: &str, status: u16, cuerpo: &str, n: usize) {
        let mut m = self.por_endpoint.lock().unwrap();
        let cola = m.entry(endpoint.to_string()).or_default();
        for _ in 0..n {
            cola.push((status, cuerpo.to_string()));
        }
    }

    /// El endpoint se cae (error de transporte).
    pub fn cae(&self, endpoint: &str) {
        self.caidas.lock().unwrap().push(endpoint.to_string());
    }

    /// Los endpoints llamados (en orden, con duplicados).
    pub fn endpoints(&self) -> Vec<String> {
        self.llamadas
            .lock()
            .unwrap()
            .iter()
            .map(|(e, _, _)| e.clone())
            .collect()
    }

    /// Cuántas veces se llamó a un endpoint.
    pub fn veces(&self, endpoint: &str) -> usize {
        self.llamadas
            .lock()
            .unwrap()
            .iter()
            .filter(|(e, _, _)| e == endpoint)
            .count()
    }

    /// El cuerpo de la petición enviada a un endpoint.
    pub fn cuerpo_enviado(&self, endpoint: &str) -> Option<Value> {
        self.llamadas
            .lock()
            .unwrap()
            .iter()
            .find(|(e, _, _)| e == endpoint)
            .map(|(_, b, _)| b.clone())
    }

    /// El timeout con el que se llamó a un endpoint.
    pub fn timeout_usado(&self, endpoint: &str) -> Option<Duration> {
        self.llamadas
            .lock()
            .unwrap()
            .iter()
            .find(|(e, _, _)| e == endpoint)
            .map(|(_, _, t)| *t)
    }
}

impl LlmTransport for MockTransport {
    fn post(&self, req: &LlmRequest, timeout: Duration) -> Result<LlmResponse, ProviderError> {
        self.llamadas
            .lock()
            .unwrap()
            .push((req.endpoint.clone(), req.body.clone(), timeout));
        if self.caidas.lock().unwrap().iter().any(|e| *e == req.endpoint) {
            return Err(ProviderError::Transport {
                provider: req.model.clone(),
                detail: "conexión rechazada (simulado)".to_string(),
            });
        }
        let mut m = self.por_endpoint.lock().unwrap();
        let cola = m.get_mut(&req.endpoint).ok_or_else(|| ProviderError::Transport {
            provider: req.model.clone(),
            detail: "sin respuesta programada".to_string(),
        })?;
        if cola.is_empty() {
            return Err(ProviderError::Transport {
                provider: req.model.clone(),
                detail: "sin respuestas disponibles".to_string(),
            });
        }
        let (status, body) = if cola.len() == 1 {
            cola[0].clone()
        } else {
            cola.remove(0)
        };
        Ok(LlmResponse { status, body })
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

/// Clave FICTICIA (no es una credencial real: sirve para comprobar que la
/// redacción la tapa).
pub const KEY: &str = "sk-EXAMPLE-NO-ES-UNA-KEY-0000";

/// Un proveedor de prueba.
pub fn prov(id: &str, endpoint: &str, model: &str, tier: &str) -> ProviderEntry {
    ProviderEntry {
        id: id.to_string(),
        name: format!("Proveedor {}", id),
        vendor: "Test".to_string(),
        endpoint: endpoint.to_string(),
        model: model.to_string(),
        api_key: KEY.to_string(),
        auth_type: "bearer".to_string(),
        tier: tier.to_string(),
        free_signup_url: String::new(),
        enabled: true,
        supports_reasoning: false,
    }
}

/// Un vault de prueba.
pub fn vault(proveedores: Vec<ProviderEntry>, default_model: &str) -> ProviderVault {
    ProviderVault {
        default_model: default_model.to_string(),
        providers: proveedores,
    }
}

/// Dependencias de prueba: sin esperas, sin reintentos, sin cuota, sin tope.
pub fn deps<'a>(v: &'a ProviderVault, t: &'a MockTransport) -> ChatDeps<'a> {
    let mut d = ChatDeps::new(v, t);
    d.quota_paused = |_| false;
    d.retries = 0;
    d.backoff_ms = 0;
    d
}

/// Cuerpo OpenAI de ejemplo (respuesta correcta).
pub const OK: &str = r#"{"choices":[{"index":0,"message":{"role":"assistant","content":"PONG"},"finish_reason":"stop"}],"usage":{"total_tokens":7}}"#;
