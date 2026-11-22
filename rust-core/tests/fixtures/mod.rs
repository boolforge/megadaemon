//! Banco de pruebas con **respuestas capturadas** de los proveedores.
//!
//! Cada fixture es un fichero JSON en `tests/fixtures/providers/` con esta
//! forma:
//!
//! ```json
//! {
//!   "provider": "minimax-m3.1-flash",
//!   "origin":   "real" | "synthetic",
//!   "source":   "de donde sale esta respuesta (medicion real o forma observada)",
//!   "status":   200,
//!   "body":     "el cuerpo EXACTO que devolvio (o devolveria) el proveedor"
//! }
//! ```
//!
//! `origin` distingue lo **medido** de lo **sintético**: ninguna respuesta
//! de este banco contiene credenciales (ni las lleva el daemon en las
//! peticiones: la key va en la cabecera `Authorization`).

use serde_json::Value;

/// Un fixture de proveedor.
#[derive(Debug, Clone)]
pub struct ProviderFixture {
    /// Nombre del fichero (sin extensión), para poder citarlo en los tests.
    pub file: &'static str,
    /// Id del proveedor al que pertenece la respuesta.
    pub provider: &'static str,
    /// `real` (medido en el dispositivo) o `synthetic` (forma observada).
    pub origin: &'static str,
    /// De dónde sale exactamente esta respuesta.
    pub source: &'static str,
    /// Status HTTP de la respuesta.
    pub status: u16,
    /// Cuerpo crudo de la respuesta.
    pub body: &'static str,
}

impl ProviderFixture {
    /// ¿La respuesta es real (medida) o sintética?
    pub fn is_real(&self) -> bool {
        self.origin == "real"
    }
}

macro_rules! fx {
    ($file:literal, $path:literal) => {{
        const RAW: &str = include_str!($path);
        let v: Value = serde_json::from_str(RAW).expect("fixture JSON invalido");
        ProviderFixture {
            file: $file,
            provider: v["provider"].as_str().unwrap_or("?"),
            origin: v["origin"].as_str().unwrap_or("?"),
            source: v["source"].as_str().unwrap_or(""),
            status: v["status"].as_u64().unwrap_or(200) as u16,
            body: v["body"].as_str().unwrap_or(""),
        }
    }};
}

/// Todos los fixtures del banco.
pub const FIXTURES: &[ProviderFixture] = &[
    fx!(
        "minimax-m3.1-flash__ok",
        "providers/minimax-m3.1-flash__ok.json"
    ),
    fx!(
        "minimax-m3.1-flash__reasoning_think",
        "providers/minimax-m3.1-flash__reasoning_think.json"
    ),
    fx!(
        "minimax-m3.1-flash__reasoning_field",
        "providers/minimax-m3.1-flash__reasoning_field.json"
    ),
    fx!(
        "openrouter-free__tool_calls",
        "providers/openrouter-free__tool_calls.json"
    ),
    fx!(
        "openrouter-free__content_array",
        "providers/openrouter-free__content_array.json"
    ),
    fx!(
        "pollinations-free__text_plain",
        "providers/pollinations-free__text_plain.json"
    ),
    fx!(
        "pollinations-free__html_error",
        "providers/pollinations-free__html_error.json"
    ),
    fx!("zai-glm-free__truncated", "providers/zai-glm-free__truncated.json"),
    fx!(
        "google-ai-studio__http_401",
        "providers/google-ai-studio__http_401.json"
    ),
    fx!(
        "minimax-m3.1-flash__http_402",
        "providers/minimax-m3.1-flash__http_402.json"
    ),
    fx!("groq-free__http_429", "providers/groq-free__http_429.json"),
    fx!("anthropic__http_500", "providers/anthropic__http_500.json"),
    fx!(
        "minimax-m3.1-flash__http_404",
        "providers/minimax-m3.1-flash__http_404.json"
    ),
    fx!(
        "ovhcloud-free__empty_choices",
        "providers/ovhcloud-free__empty_choices.json"
    ),
    fx!("local-seere__offline", "providers/local-seere__offline.json"),
];

/// Busca un fixture por el nombre de su fichero (sin extensión).
pub fn by_file(file: &str) -> Option<&'static ProviderFixture> {
    FIXTURES.iter().find(|f| f.file == file)
}

/// Busca un fixture por proveedor y status.
pub fn by_provider_status(provider: &str, status: u16) -> Option<&'static ProviderFixture> {
    FIXTURES.iter().find(|f| f.provider == provider && f.status == status)
}
