//! F3 — tabla de precios de la flota, **con su fuente**.
//!
//! Regla del proyecto: **no se inventa una cifra**.  Cada entrada guarda la
//! URL oficial y la fecha de captura; lo que no se puede citar **no entra**
//! ⇒ el `PriceBook` lo trata como *desconocido* y el router lo penaliza
//! (`NO VERIFICADO`).
//!
//! Las tarifas oficiales se publican **por 1M de tokens**; el `PriceBook`
//! trabaja **por 1K** ⇒ la conversión es ÷1000 (aquí está el error clásico:
//! confundir 1M con 1K ⇒ 1000x de más en el coste).
//!
//! Fuente de la tabla: [`docs/F3_PRECIOS.md`](../../docs/F3_PRECIOS.md).

use crate::agent_loop::{Cost, PriceBook};

/// Fecha de captura de las fuentes de esta tabla.
pub const CAPTURED_AT: &str = "2026-11-20";

/// Fuente oficial de OpenAI (tarifas por 1M tokens).
pub const SRC_OPENAI: &str = "https://platform.openai.com/docs/pricing";
/// Fuente oficial de Anthropic (tarifas por MTok).
pub const SRC_ANTHROPIC: &str = "https://docs.anthropic.com/en/docs/about-claude/pricing";
/// Fuente oficial de Google AI Studio (tarifas por 1M tokens).
pub const SRC_GOOGLE: &str = "https://aistudio.google.com/docs/pricing";
/// Fuente oficial de MiniMax (tarifas por M tokens).
pub const SRC_MINIMAX: &str = "https://platform.minimax.io/docs/pricing/overview";
/// Fuente oficial de DeepSeek (tarifas por 1M tokens).
pub const SRC_DEEPSEEK: &str = "https://api-docs.deepseek.com/quick_start/pricing";

/// Una tarifa de la tabla, tal y como la publica el proveedor.
#[derive(Clone, Debug)]
pub struct FleetPrice {
    pub model: &'static str,
    /// USD por **1M** de tokens de entrada (tal cual lo publica el proveedor).
    pub usd_per_1m_input: f64,
    /// USD por **1M** de tokens de salida.
    pub usd_per_1m_output: f64,
    /// URL de la página oficial.
    pub source: &'static str,
    /// Matiz (precio introductorio, franja horaria, longitud de prompt...).
    pub note: &'static str,
}

/// La tabla completa de la flota, con fuente y fecha.
pub const FLEET_PRICES: &[FleetPrice] = &[
    // ---------------- OpenAI (por 1M tokens) ----------------
    FleetPrice { model: "gpt-6.2-astra", usd_per_1m_input: 8.00, usd_per_1m_output: 40.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-6.2-sol", usd_per_1m_input: 2.00, usd_per_1m_output: 10.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-6-astra", usd_per_1m_input: 10.00, usd_per_1m_output: 50.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-6.1-sol", usd_per_1m_input: 2.00, usd_per_1m_output: 10.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-5.6-sol", usd_per_1m_input: 4.00, usd_per_1m_output: 20.00, source: SRC_OPENAI, note: "precio estandar" },
    FleetPrice { model: "gpt-5.6-terra", usd_per_1m_input: 2.00, usd_per_1m_output: 12.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-5.6-luna", usd_per_1m_input: 0.20, usd_per_1m_output: 1.20, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-5.5", usd_per_1m_input: 12.50, usd_per_1m_output: 75.00, source: SRC_OPENAI, note: "prompt < 272k tokens" },
    FleetPrice { model: "gpt-5.4", usd_per_1m_input: 5.00, usd_per_1m_output: 30.00, source: SRC_OPENAI, note: "prompt < 272k tokens" },
    FleetPrice { model: "gpt-5.4-mini", usd_per_1m_input: 0.75, usd_per_1m_output: 4.50, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-5.4-nano", usd_per_1m_input: 0.20, usd_per_1m_output: 1.25, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-5.3", usd_per_1m_input: 1.75, usd_per_1m_output: 14.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-5.2", usd_per_1m_input: 1.75, usd_per_1m_output: 14.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-5.1", usd_per_1m_input: 1.25, usd_per_1m_output: 10.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-5", usd_per_1m_input: 1.25, usd_per_1m_output: 10.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-5-mini", usd_per_1m_input: 0.25, usd_per_1m_output: 2.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-5-nano", usd_per_1m_input: 0.05, usd_per_1m_output: 0.40, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-4.1", usd_per_1m_input: 2.00, usd_per_1m_output: 8.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-4.1-mini", usd_per_1m_input: 0.40, usd_per_1m_output: 1.60, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-4.1-nano", usd_per_1m_input: 0.05, usd_per_1m_output: 0.20, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-4o", usd_per_1m_input: 2.50, usd_per_1m_output: 10.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "gpt-4o-mini", usd_per_1m_input: 0.15, usd_per_1m_output: 0.60, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "o3", usd_per_1m_input: 2.00, usd_per_1m_output: 8.00, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "o3-mini", usd_per_1m_input: 1.10, usd_per_1m_output: 4.40, source: SRC_OPENAI, note: "" },
    FleetPrice { model: "o4-mini", usd_per_1m_input: 1.10, usd_per_1m_output: 4.40, source: SRC_OPENAI, note: "" },
    // ---------------- Anthropic (por MTok) ----------------
    FleetPrice { model: "claude-sonnet-5-5", usd_per_1m_input: 2.00, usd_per_1m_output: 10.00, source: SRC_ANTHROPIC, note: "" },
    FleetPrice { model: "claude-opus-5-5", usd_per_1m_input: 4.00, usd_per_1m_output: 20.00, source: SRC_ANTHROPIC, note: "" },
    FleetPrice { model: "claude-haiku-5-5", usd_per_1m_input: 0.10, usd_per_1m_output: 0.50, source: SRC_ANTHROPIC, note: "prompt <= 100k tokens" },
    FleetPrice { model: "claude-sonnet-5", usd_per_1m_input: 2.00, usd_per_1m_output: 10.00, source: SRC_ANTHROPIC, note: "" },
    FleetPrice { model: "claude-opus-5", usd_per_1m_input: 5.00, usd_per_1m_output: 25.00, source: SRC_ANTHROPIC, note: "" },
    FleetPrice { model: "claude-opus-4-8", usd_per_1m_input: 5.00, usd_per_1m_output: 25.00, source: SRC_ANTHROPIC, note: "" },
    FleetPrice { model: "claude-haiku-4-5", usd_per_1m_input: 1.00, usd_per_1m_output: 5.00, source: SRC_ANTHROPIC, note: "" },
    // ---------------- Google (por 1M tokens) ----------------
    FleetPrice { model: "gemini-3.8-flash", usd_per_1m_input: 1.50, usd_per_1m_output: 7.50, source: SRC_GOOGLE, note: "precio estandar; introductorio 0.75/3.75 hasta 2026-12-31" },
    // ---------------- MiniMax (por M tokens) ----------------
    FleetPrice { model: "minimax-m3.1-flash", usd_per_1m_input: 0.15, usd_per_1m_output: 0.60, source: SRC_MINIMAX, note: "tarifa estandar" },
    FleetPrice { model: "minimax-m3", usd_per_1m_input: 0.30, usd_per_1m_output: 1.20, source: SRC_MINIMAX, note: "prompt <= 512k; descuento permanente del 50%" },
    // ---------------- DeepSeek (por 1M tokens) ----------------
    FleetPrice { model: "deepseek-flash", usd_per_1m_input: 0.15, usd_per_1m_output: 0.60, source: SRC_DEEPSEEK, note: "cache miss, franja valle (off-peak); pico x2" },
    FleetPrice { model: "deepseek-v4-pro", usd_per_1m_input: 0.66, usd_per_1m_output: 1.98, source: SRC_DEEPSEEK, note: "cache miss, franja valle (off-peak); pico x2" },
];

/// Modelos que la flota usa y **no** se pueden cotizar con una fuente
/// citable ⇒ quedan como *desconocidos* (el router los penaliza).
pub const UNPRICED_WITH_REASON: &[(&str, &str)] = &[
    ("kimi-k3", "el proveedor publica en CNY y no hay fuente de tipo de cambio"),
    ("kimi-k3.1-pro", "el proveedor publica en CNY y no hay fuente de tipo de cambio"),
    ("mistral-small-latest", "sin pagina de precios oficial consultada en esta captura"),
    ("mistral-free", "sin pagina de precios oficial consultada en esta captura"),
    ("grok", "sin pagina de precios oficial consultada en esta captura"),
    ("gpt-oss-120b", "pesos abiertos: se sirve en local, no hay precio por API"),
    ("claude-code", "alias de cliente, no es un modelo facturable"),
];

/// Construye el `PriceBook` de la flota con las fuentes citadas.
pub fn fleet_price_book() -> PriceBook {
    let mut pb = PriceBook::new();
    for p in FLEET_PRICES {
        pb = pb.with_source(
            p.model,
            p.usd_per_1m_input / 1000.0,
            p.usd_per_1m_output / 1000.0,
            &format!("{} (capturado {}){}", p.source, CAPTURED_AT, if p.note.is_empty() { String::new() } else { format!(" - {}", p.note) }),
        );
    }
    // Los que no se pueden citar se registran SIN precio => el `PriceBook` los
    // devuelve como NO VERIFICADO (no como 0).
    for (model, _reason) in UNPRICED_WITH_REASON {
        pb = pb.without_source(model);
    }
    pb
}

/// Explicación legible de por qué un modelo no tiene precio.
pub fn unpriced_reason(model: &str) -> Option<&'static str> {
    UNPRICED_WITH_REASON
        .iter()
        .find(|(m, _)| *m == model)
        .map(|(_, r)| *r)
}

/// Coste de una llamada con el libro de la flota.
pub fn fleet_cost(model: &str, tokens_in: u64, tokens_out: u64) -> Cost {
    fleet_price_book().cost_of(model, tokens_in, tokens_out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_conversion_de_1m_a_1k_es_correcta() {
        // gpt-4o-mini: 0.15 USD / 1M entrada y 0.60 USD / 1M salida
        // => 0.00015 + 0.0006 USD por 1K de cada uno.
        // Una llamada de 1.000 tokens de entrada y 1.000 de salida cuesta
        // 0.00015 + 0.0006 = 0.00075 USD.
        let pb = fleet_price_book();
        let c = pb.cost_of("gpt-4o-mini", 1_000, 1_000);
        assert!(c.is_verified(), "debe tener precio verificable");
        assert!((c.usd.unwrap() - 0.00075).abs() < 1e-12, "coste: {:?}", c.usd);
        // El source lleva la URL oficial y la fecha.
        let src = c.source.unwrap();
        assert!(src.contains("platform.openai.com/docs/pricing"), "src: {}", src);
        assert!(src.contains(CAPTURED_AT), "src: {}", src);
    }

    #[test]
    fn lo_sin_fuente_queda_no_verificado_y_no_es_cero() {
        let pb = fleet_price_book();
        let c = pb.cost_of("kimi-k3", 1000, 1000);
        assert!(!c.is_verified());
        assert!(c.usd.is_none(), "nunca 0 como si fuera gratis");
        assert!(c.note.contains("NO VERIFICADO"));
        assert!(unpriced_reason("kimi-k3").unwrap().contains("CNY"));
        // Y un modelo que no existe tampoco.
        let c2 = pb.cost_of("modelo-inventado", 1000, 1000);
        assert!(c2.usd.is_none());
    }

    #[test]
    fn el_libro_cubre_los_modelos_de_la_flota_con_precio() {
        let pb = fleet_price_book();
        for m in [
            "gpt-6.2-sol",
            "gpt-5.6-sol",
            "claude-sonnet-5-5",
            "gemini-3.8-flash",
            "minimax-m3.1-flash",
            "deepseek-flash",
        ] {
            let c = pb.cost_of(m, 1000, 1000);
            assert!(c.is_verified(), "{} deberia tener precio", m);
        }
    }
}
