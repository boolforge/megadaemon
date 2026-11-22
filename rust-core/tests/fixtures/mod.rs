//! Banco de respuestas CAPTURADAS de proveedores (MegaDaemon v16).
//!
//! QUÉ ES ESTO
//! -----------
//! Un conjunto de **cuerpos de respuesta congelados** tal y como los
//! devuelve cada proveedor (o similar), para poder ejercitar el parseo y
//! la cascada **sin red** y **sin secretos**.
//!
//! Cada fichero tiene esta forma:
//!
//! ```json
//! {
//!   "_meta": {
//!     "id": "minimax-m3.1-flash__ok",
//!     "provider": "minimax-m3.1-flash",
//!     "status": 200,
//!     "origen": "SINTETICO",
//!     "nota": "de donde sale y que caso raro cubre"
//!   },
//!   "body": { ... }        // cuerpo JSON del proveedor
//! }
//! ```
//!
//! ...o, cuando el proveedor devuelve algo que NO es JSON:
//!
//! ```json
//! {
//!   "_meta": { ... },
//!   "raw": "<html>...502...</html>"
//! }
//! ```
//!
//! PROCEDENCIA (OBLIGATORIA)
//! -------------------------
//! * `SINTETICO`: la FORMA es la documentada por el proveedor, pero el
//!   cuerpo se compuso aquí a mano.  Es lo honesto: en esta sesión no se
//!   capturó ninguna traza HTTP cruda de un proveedor real.
//! * `REAL`: el cuerpo se copió literal de una captura real.  En este banco
//!   **no hay ninguno**: la única medición real del 2026-11-22 fue la
//!   salida del daemon en el móvil del usuario
//!   (`{"reply":"MINIMAX_OK","vendor":"MiniMax","latency_ms":1232.0,
//!   "fallback_used":false}`), que es la salida del daemon, no el cuerpo
//!   HTTP del proveedor; por eso la nota del fixture `minimax-m3.1-flash__ok`
//!   lo dice así.
//!
//! SEGURIDAD
//! ---------
//! **Ninguna credencial real.**  Las claves que aparecen en los cuerpos de
//! error son FICTICIAS (`sk-EXAMPLE-NO-ES-UNA-KEY-0000`): están para
//! comprobar que la redaction las tapa antes de que lleguen al cliente.

use serde_json::Value;

/// Una respuesta de proveedor congelada, con su procedencia.
#[derive(Debug, Clone, Copy)]
pub struct Fixture {
    /// Nombre del fichero dentro de `tests/fixtures/providers/`.
    pub file: &'static str,
    /// Id del proveedor del que "viene" la respuesta.
    pub provider: &'static str,
    /// Status HTTP con el que respondió.
    pub status: u16,
    /// Procedencia: `REAL` o `SINTETICO`.
    pub origen: &'static str,
    /// Qué caso raro cubre.
    pub caso: &'static str,
}

/// Índice del banco (fuente de verdad para los tests).
pub const FIXTURES: &[Fixture] = &[
    Fixture {
        file: "minimax-m3.1-flash__ok.json",
        provider: "minimax-m3.1-flash",
        status: 200,
        origen: "SINTETICO",
        caso: "respuesta normal (content string + usage)",
    },
    Fixture {
        file: "minimax-m3.1-flash__reasoning_field.json",
        provider: "minimax-m3.1-flash",
        status: 200,
        origen: "SINTETICO",
        caso: "razonamiento en el campo 'reasoning_content'",
    },
    Fixture {
        file: "minimax-m3.1-flash__reasoning_think.json",
        provider: "minimax-m3.1-flash",
        status: 200,
        origen: "SINTETICO",
        caso: "razonamiento dentro del texto con <think>...</think>",
    },
    Fixture {
        file: "minimax-m3.1-flash__http_401.json",
        provider: "minimax-m3.1-flash",
        status: 401,
        origen: "SINTETICO",
        caso: "401 que ECOA la api key (prueba de la redaccion)",
    },
    Fixture {
        file: "minimax-m3.1-flash__http_402.json",
        provider: "minimax-m3.1-flash",
        status: 402,
        origen: "SINTETICO",
        caso: "402 sin saldo (cuota: hay que registrarla)",
    },
    Fixture {
        file: "minimax-m3.1-flash__http_404.json",
        provider: "minimax-m3.1-flash",
        status: 404,
        origen: "SINTETICO",
        caso: "404 modelo inexistente (NO se reintenta)",
    },
    Fixture {
        file: "groq-free__http_429.json",
        provider: "groq-free",
        status: 429,
        origen: "SINTETICO",
        caso: "429 con 'resets at' y 'retry after' (pausa de cuota)",
    },
    Fixture {
        file: "google-ai-studio__http_401.json",
        provider: "google-ai-studio",
        status: 401,
        origen: "SINTETICO",
        caso: "401 con el envoltorio de error de Google",
    },
    Fixture {
        file: "anthropic__http_500.json",
        provider: "anthropic",
        status: 500,
        origen: "SINTETICO",
        caso: "500 (SI se reintenta con backoff)",
    },
    Fixture {
        file: "openrouter-free__tool_calls.json",
        provider: "openrouter-free",
        status: 200,
        origen: "SINTETICO",
        caso: "tool_calls nativas (arguments como texto JSON)",
    },
    Fixture {
        file: "openrouter-free__tool_calls_bad_arguments.json",
        provider: "openrouter-free",
        status: 200,
        origen: "SINTETICO",
        caso: "tool_calls con 'arguments' no JSON (no debe entrar en panic)",
    },
    Fixture {
        file: "openrouter-free__content_array.json",
        provider: "openrouter-free",
        status: 200,
        origen: "SINTETICO",
        caso: "'message.content' como ARRAY de partes",
    },
    Fixture {
        file: "ovhcloud-free__empty_choices.json",
        provider: "ovhcloud-free",
        status: 200,
        origen: "SINTETICO",
        caso: "2xx con 'choices' vacio (error con mensaje util)",
    },
    Fixture {
        file: "pollinations-free__html_error.json",
        provider: "pollinations-free",
        status: 502,
        origen: "SINTETICO",
        caso: "cuerpo HTML (no JSON) en un error",
    },
    Fixture {
        file: "pollinations-free__text_plain.json",
        provider: "pollinations-free",
        status: 200,
        origen: "SINTETICO",
        caso: "respuesta en texto plano (endpoint zero-key)",
    },
    Fixture {
        file: "pollinations-free__text_plain_utf8.json",
        provider: "pollinations-free",
        status: 200,
        origen: "SINTETICO",
        caso: "texto plano UTF-8 multibyte (recorte sin panic)",
    },
    Fixture {
        file: "zai-glm-free__truncated.json",
        provider: "zai-glm-free",
        status: 200,
        origen: "SINTETICO",
        caso: "respuesta truncada (JSON invalido)",
    },
    Fixture {
        file: "openai-compatible__legacy_text.json",
        provider: "openai-compatible",
        status: 200,
        origen: "SINTETICO",
        caso: "forma legacy 'choices[0].text'",
    },
    Fixture {
        file: "openai-compatible__error_in_200.json",
        provider: "openai-compatible",
        status: 200,
        origen: "SINTETICO",
        caso: "2xx con cuerpo de error dentro",
    },
    Fixture {
        file: "local-seere__offline.json",
        provider: "local-seere",
        status: 200,
        origen: "SINTETICO",
        caso: "forma propia del SEERE local (no es OpenAI)",
    },
];

/// Ruta absoluta del fichero de un fixture.
pub fn ruta(file: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("providers")
        .join(file)
}

/// El fichero completo (con `_meta`).
pub fn fichero(file: &str) -> Value {
    let txt = std::fs::read_to_string(ruta(file))
        .unwrap_or_else(|e| panic!("no se pudo leer el fixture {}: {}", file, e));
    serde_json::from_str(&txt)
        .unwrap_or_else(|e| panic!("el fixture {} no es JSON valido: {}", file, e))
}

/// El cuerpo CRUDO tal y como lo devolvio el proveedor.
pub fn cuerpo(file: &str) -> String {
    let v = fichero(file);
    if let Some(s) = v.get("raw").and_then(|x| x.as_str()) {
        return s.to_string();
    }
    match v.get("body") {
        Some(b) => serde_json::to_string(b).unwrap_or_default(),
        None => String::new(),
    }
}
