//! MegaDaemon v13.4 — Motor de delegación a cuentas gratuitas ("enjambre
//! oportunista").
//!
//! QUÉ RESUELVE
//!
//! El usuario tiene cuentas **gratuitas** de cloud (Claude, Kimi, MiniMax,
//! DeepSeek, ChatGPT/Codex y Gemini con planes Google One/familiar) y quiere
//! **delegar** trabajo a esas cuentas de forma oportunista/parásita para
//! ahorrar tokens, revisando después los defectos y auto-mejitando el
//! enrutado.  Este módulo es el *cerebro* de esa delegación:
//!
//!   * **cola** de tareas delegables (`submit`),
//!   * **enrutado determinista** que prefiere siempre la cuenta gratuita
//!     adecuada y reserva las cuentas de pago como último recurso
//!     (`pick_account`), incluyendo *learning* por cuenta (EWMA de calidad),
//!   * **contabilidad** de tokens, coste estimado y éxito por cuenta
//!     (`stats`),
//!   * **plantillas de prompt** por tipo de tarea (`prompt_template`),
//!   * **persistencia JSON** de las cuentas con validación estricta
//!     (`load_accounts` / `save_accounts`),
//!   * **auto-mejora**: de las estadísticas se derivan recomendaciones
//!     accionables (`adjustments`) que se aplican validando
//!     (`apply_adjustments`), cerrando el bucle
//!     *delegar → medir → corregir → volver a delegar*.
//!
//! MegaDaemon sigue siendo el **orquestador**: este módulo NO habla con
//! ningún proveedor.  Decide *a quién* delegar, *en qué orden* y *cuánto*
//! cuesta; la ejecución y la ingesta del resultado son de otro módulo.  Por
//! eso todo el enrutado es **puro** (misma entrada → misma salida) y testeable
//! sin red.
//!
//! MODELO DE COSTE
//!
//! Hay dos costes distintos y no deben confundirse:
//!
//!   * **coste de lista** (`estimate_cost_usd`): lo que cuesta el modelo por
//!     la vía API de pago (USD por millón de tokens).  Es el "precio de
//!     sombra" de la delegación;
//!   * **coste efectivo**: lo que le cuesta *de verdad* al usuario.  Una
//!     cuenta de tier gratuito cuesta **0 USD** aunque el modelo de lista sea
//!     caro (la cuota ya está pagada por la suscripción).  El ahorro real del
//!     enjambre es exactamente la diferencia entre ambos.
//!
//! Los precios de la tabla están verificados con `web_search` el **2026-11-19**
//! contra las páginas oficiales de cada proveedor (ver el bloque de fuentes
//! sobre `PRICING_TABLE`).  Los modelos gratuitos (tier free del proveedor,
//! modelos locales o marcados `:free`) cuestan 0.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// Tope duro del presupuesto diario por cuenta (USD).  Ninguna cuenta puede
/// declarar más presupuesto que esto: es el "techo" de la validación.
pub const MAX_DAILY_BUDGET_USD: f64 = 25.0;

/// Rango de prioridad admitido por cuenta (mayor = preferida).
pub const MAX_PRIORITY: i32 = 100;

/// Intentos máximos por tarea antes de darla por fallida.
pub const MAX_ATTEMPTS: usize = 3;

/// Factor de suavizado (α) de la EWMA de calidad/éxito.
pub const EWMA_ALPHA: f32 = 0.30;

/// Prior neutro de calidad: una cuenta sin historial vale 0.5 (ni buena ni
/// mala), para que un solo éxito la mejore y un solo fallo laCastigue.
pub const QUALITY_PRIOR: f32 = 0.5;

// ─────────────────────────────────────────────────────────────────────────────
// Tipos públicos
// ─────────────────────────────────────────────────────────────────────────────

/// Una cuenta de proveedor (normalmente gratuita) con su cuota y su
/// presupuesto diario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderAccount {
    /// Identificador estable y único (`kimi-web`, `gemini-family`, ...).
    pub id: String,
    /// Familia de proveedor (`kimi`, `minimax`, `deepseek`, `anthropic`, ...).
    pub provider: String,
    /// Nivel de la cuenta: `free`, `trial`, `family`, `subscription`, `paid`,
    /// `prepaid`, `local`.
    pub tier: String,
    /// Modelo concreto que se usará en esta cuenta.
    pub model: String,
    /// Capacidades que la cuenta puede resolver (`research`, `code`, ...).
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Peticiones que quedan en la cuota; `None` = cuota no medida.
    #[serde(default)]
    pub remaining_quota: Option<u32>,
    /// Presupuesto diario en USD (techo de gasto real de la cuenta).
    #[serde(default)]
    pub daily_budget_usd: f64,
    /// Prioridad declarada por el usuario (mayor = preferida).
    #[serde(default)]
    pub priority: i32,
    /// Si la cuenta está habilitada para recibir tareas.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Notas libres (dónde está la cuenta, cómo se usa, etc.).
    #[serde(default)]
    pub notes: String,
}

fn default_enabled() -> bool {
    true
}

/// Una tarea delegable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DelegationTask {
    /// Identificador de la tarea (se genera si viene vacío).
    pub id: String,
    /// Texto a delegar.
    pub prompt: String,
    /// Tipo de tarea (`research`, `writing`, `code`, `summary`, `audit`,
    /// `classification`): decide la plantilla de prompt.
    #[serde(default)]
    pub kind: String,
    /// Capacidades que la cuenta debe tener (todas, no cualquiera).
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    /// Tope de tokens de salida pedido al proveedor.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Deadline (ISO-8601) si lo hay; ordena el plan.
    #[serde(default)]
    pub deadline: Option<String>,
    /// `queued` | `running` | `retry` | `done` | `failed`.
    #[serde(default)]
    pub status: String,
    /// Intentos ya registrados.
    #[serde(default)]
    pub attempts: Vec<Attempt>,
    /// Cuenta a la que se asignó (última asignación).
    #[serde(default)]
    pub assigned_account: Option<String>,
}

fn default_max_tokens() -> u32 {
    1024
}

/// Un intento de ejecución de una tarea en una cuenta concreta.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attempt {
    /// Cuenta que ejecutó el intento.
    pub account: String,
    /// Modelo usado en el intento.
    pub model: String,
    /// Inicio (ISO-8601).
    pub started_at: String,
    /// Fin (ISO-8601), si terminó.
    #[serde(default)]
    pub finished_at: Option<String>,
    /// Si el intento tuvo éxito.
    #[serde(default)]
    pub ok: bool,
    /// Tokens de entrada consumidos.
    #[serde(default)]
    pub input_tokens: u32,
    /// Tokens de salida consumidos.
    #[serde(default)]
    pub output_tokens: u32,
    /// Error, si lo hubo.
    #[serde(default)]
    pub error: Option<String>,
    /// Calidad revisada (0.0..=1.0) de la respuesta.
    #[serde(default)]
    pub quality_score: Option<f32>,
}

/// Un paso del plan de ejecución de la cola.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlanStep {
    /// Orden de ejecución (1-based).
    pub order: usize,
    /// Tarea que ejecuta este paso.
    pub task_id: String,
    /// Tipo de la tarea.
    pub kind: String,
    /// Cuenta elegida (vacío si no hay ninguna posible).
    pub account: String,
    /// Modelo que usará la cuenta.
    pub model: String,
    /// Tokens de entrada estimados.
    pub estimated_input_tokens: u32,
    /// Tokens de salida estimados.
    pub estimated_output_tokens: u32,
    /// Coste efectivo estimado (0 para cuentas gratuitas).
    pub estimated_cost_usd: f64,
    /// Por qué se eligió esta cuenta (razón legible).
    pub reason: String,
}

/// Acción de auto-mejora aplicable a una cuenta.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", content = "value", rename_all = "snake_case")]
pub enum AdjustmentAction {
    /// Activar la cuenta.
    Enable,
    /// Desactivar la cuenta.
    Disable,
    /// Fijar la prioridad (rango ±`MAX_PRIORITY`).
    SetPriority(i32),
    /// Fijar el presupuesto diario (0.0 ..= `MAX_DAILY_BUDGET_USD`).
    SetBudgetUsd(f64),
    /// Cambiar el modelo de la cuenta.
    SetModel(String),
}

/// Recomendación accionable derivada de las estadísticas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Adjustment {
    /// Cuenta sobre la que actuar.
    pub account: String,
    /// Qué hacer.
    pub action: AdjustmentAction,
    /// Por qué (razón legible).
    pub reason: String,
}

/// Instantánea de las estadísticas de una cuenta.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct AccountStats {
    pub account: String,
    pub provider: String,
    pub tier: String,
    pub model: String,
    pub capabilities: Vec<String>,
    pub priority: i32,
    pub enabled: bool,
    pub attempts: u32,
    pub ok: u32,
    pub failed: u32,
    pub consecutive_failures: u32,
    /// EWMA de calidad/éxito (0.0..=1.0).
    pub score: f32,
    /// Tasa de éxito (0.0..=1.0).
    pub success_rate: f32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Coste efectivo acumulado (USD) — 0 para cuentas gratuitas.
    pub cost_usd: f64,
    /// Coste de lista acumulado (lo que costaría por la vía de pago).
    pub shadow_cost_usd: f64,
    pub remaining_quota: Option<u32>,
    pub daily_budget_usd: f64,
    /// La cuenta ha agotado su cuota.
    pub burned: bool,
}

/// Aprendizaje por cuenta (EWMA + contadores).  Es estado interno del motor.
#[derive(Clone, Debug, Default)]
struct AccountLearning {
    attempts: u32,
    ok: u32,
    failed: u32,
    consecutive_failures: u32,
    ewma: f32,
    input_tokens: u64,
    output_tokens: u64,
    cost_usd: f64,
    shadow_cost_usd: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Precios
// ─────────────────────────────────────────────────────────────────────────────

/// Tabla de precios de lista por millón de tokens (USD).
///
/// FUENTES (consultadas con `web_search` el **2026-11-19**):
///
/// * Anthropic — https://docs.anthropic.com/en/docs/about-claude/pricing
///   (tarifa vigente 2026-10-22:
///   https://assets.claude.com/5633dc848caa431115468f31c7085d695bbfb9cc.pdf)
/// * OpenAI — https://developers.openai.com/api/docs/pricing
/// * Google Gemini — https://ai.google.dev/gemini-api/docs/pricing
///   (tier free: "Free of charge" → 0 USD)
/// * Moonshot Kimi — https://platform.kimi.ai/docs/pricing/chat
/// * MiniMax — https://platform.minimax.io/docs/pricing/overview
/// * DeepSeek — https://api-docs.deepseek.com/quick_start/pricing
///
/// Formato: `(prefijo de modelo normalizado, $/MTok entrada, $/MTok salida,
/// fuente)`.  El prefijo más largo que case gana (`gpt-5.5` antes que `gpt-5`).
///
/// NOTAS DE LA TABLA:
/// * **DeepSeek** se guarda a tarifa **pico** (la Conservative): fuera de
///   pico son la mitad ($0.15/$0.60 el Flash, $0.66/$1.98 el Pro).  Los
///   alias `deepseek-chat`/`deepseek-reasoner` se retiraron el 2026-07-24;
///   se mantienen en la tabla al precio del Flash actual (`deepseek-flash`).
/// * **MiniMax-M3** es $0.30/$1.20 hasta 512k de entrada y $0.60/$2.40 por
///   encima (contexto largo); aquí se usa la tarifa corta, la habitual.
/// * **Gemini** tiene tier free (0 USD) en casi todos los modelos: el
///   consumo gratis depende de la *cuenta*, no del modelo, así que aquí se
///   guarda el precio del tier de pago y el motor lo pone a 0 cuando la
///   cuenta es gratuita.
/// * `gpt-5.6-sol` tiene precio promocional ($4/$20) hasta 2026-11-30.
pub const PRICING_TABLE: &[(&str, f64, f64, &str)] = &[
    // ---- Anthropic (docs.anthropic.com, tarifa 2026-10-22) ----
    ("claude-opus-5.5", 4.00, 20.00, "anthropic"),
    ("claude-sonnet-5.5", 2.00, 10.00, "anthropic"),
    ("claude-haiku-5.5", 0.10, 0.50, "anthropic"),
    ("claude-fable-5.2", 8.00, 40.00, "anthropic"),
    ("claude-fable-5.1", 10.00, 50.00, "anthropic"),
    ("claude-mythos-5.2", 8.00, 40.00, "anthropic"),
    ("claude-opus-5", 5.00, 25.00, "anthropic"),
    ("claude-opus-4.8", 5.00, 25.00, "anthropic"),
    ("claude-opus-4.5", 5.00, 25.00, "anthropic"),
    ("claude-opus-4.1", 15.00, 75.00, "anthropic"),
    ("claude-sonnet-5", 2.00, 10.00, "anthropic"),
    ("claude-sonnet-4.6", 3.00, 15.00, "anthropic"),
    ("claude-sonnet-4.5", 3.00, 15.00, "anthropic"),
    ("claude-sonnet-4", 3.00, 15.00, "anthropic"),
    ("claude-haiku-4.5", 1.00, 5.00, "anthropic"),
    // ---- OpenAI (developers.openai.com) ----
    ("gpt-6.2-astra", 8.00, 40.00, "openai"),
    ("gpt-6.2-sol", 2.00, 10.00, "openai"),
    ("gpt-6-astra", 10.00, 50.00, "openai"),
    ("gpt-6.1-sol", 2.00, 10.00, "openai"),
    ("gpt-6-luna", 0.10, 0.50, "openai"),
    ("gpt-5.6-sol", 4.00, 20.00, "openai"),
    ("gpt-5.5", 5.00, 30.00, "openai"),
    ("gpt-5.4", 1.25, 7.50, "openai"),
    ("gpt-5.1", 2.50, 20.00, "openai"),
    ("gpt-5", 1.25, 10.00, "openai"),
    ("gpt-5-mini", 0.25, 2.00, "openai"),
    ("gpt-5-nano", 0.05, 0.40, "openai"),
    ("gpt-4.1", 2.00, 8.00, "openai"),
    ("gpt-4o", 2.50, 10.00, "openai"),
    ("gpt-4o-mini", 0.15, 0.60, "openai"),
    ("codex-mini-latest", 1.50, 6.00, "openai"),
    // ---- Google Gemini (ai.google.dev) — tier de pago; el free es 0 ----
    // Re-verificado el 2026-11-19 contra la página oficial de precios: los
    // valores anteriores (1.50/9.00, 0.30/2.50, 0.10/0.40) estaban
    // desfasados.  Ahora: 3.1 Pro = $2/$12 (≤200k) y $4/$18 (>200k),
    // 3 Flash = $0.50/$3, 3.1 Flash-Lite = $0.25/$1.50, 3.8 Flash =
    // $0.75/$3.75 hasta 2026-12-31 (luego $1.50/$7.50).
    ("gemini-3-pro", 2.00, 12.00, "google"),
    ("gemini-3-flash", 0.50, 3.00, "google"),
    ("gemini-3-flash-lite", 0.25, 1.50, "google"),
    ("gemini-3.1-pro-preview", 2.00, 12.00, "google"),
    ("gemini-3.8-flash", 0.75, 3.75, "google"),
    ("gemini-flash", 0.50, 3.00, "google"),
    ("gemini-flash-lite", 0.25, 1.50, "google"),
    // ---- Moonshot Kimi (platform.kimi.ai) ----
    ("kimi-k3.1-pro", 2.00, 10.00, "moonshot"),
    ("kimi-k3", 3.00, 15.00, "moonshot"),
    ("kimi-k2.7-code", 0.95, 4.00, "moonshot"),
    ("kimi-k2.6", 0.95, 4.00, "moonshot"),
    ("kimi-k2.5", 0.60, 3.00, "moonshot"),
    // ---- MiniMax (platform.minimax.io) ----
    ("minimax-m3.1-flash", 0.15, 0.60, "minimax"),
    ("minimax-m3", 0.30, 1.20, "minimax"),
    // ---- DeepSeek (api-docs.deepseek.com) — tarifa pico, conservadora ----
    ("deepseek-v4-pro", 1.32, 3.96, "deepseek"),
    ("deepseek-flash", 0.30, 1.20, "deepseek"),
    ("deepseek-chat", 0.30, 1.20, "deepseek"),
    ("deepseek-reasoner", 0.30, 1.20, "deepseek"),
];

// ─────────────────────────────────────────────────────────────────────────────
// Procedencia de los precios: fuente oficial + fecha de verificación
// ─────────────────────────────────────────────────────────────────────────────
//
// Un precio sin fuente no es un dato: es una suposición.  Por eso cada
// modelo de la tabla hereda de su proveedor (a) la **URL oficial** donde se
// leyó el precio y (b) la **fecha** en que se leyó.  Con eso el motor puede
// avisar de los precios rancios (`pricing_stale`) en vez de seguir estimando
// el presupuesto "a ojo", que es exactamente lo que el §8 del plan maestro
// marca como decisión pendiente ("el enrutado debe medirse: coste real, no
// estimado a ojo").

/// Fecha de la última verificación global de la tabla de precios.
pub const PRICING_VERIFIED_ON: &str = "2026-11-19";

/// Fuente oficial por proveedor: `(proveedor, url, verificado_el)`.
///
/// Las seis fuentes se re-verificaron el **2026-11-19** contra las páginas
/// oficiales de cada proveedor.  Los precios de MiniMax, DeepSeek y Kimi son
/// **tarifa directa del proveedor**: OpenRouter (y otros revendedores) cobran
/// otra cosa, así que comparar contra un agregador da falsos positivos.
pub const PRICING_SOURCES: &[(&str, &str, &str)] = &[
    ("anthropic", "https://platform.claude.com/docs/en/about-claude/pricing", "2026-11-19"),
    ("openai", "https://developers.openai.com/api/docs/pricing", "2026-11-19"),
    ("google", "https://ai.google.dev/gemini-api/docs/pricing", "2026-11-19"),
    ("moonshot", "https://platform.kimi.ai/docs/pricing", "2026-11-19"),
    ("minimax", "https://platform.minimax.io/docs/pricing/overview", "2026-11-19"),
    ("deepseek", "https://api-docs.deepseek.com/quick_start/pricing", "2026-11-19"),
];

/// Excepciones por modelo: `(modelo, url, verificado_el, nota)`.
///
/// Se usa cuando el precio de un modelo concreto difiere del de su
/// proveedor (promociones temporales, tarifa de contexto largo, ...).
pub const PRICING_OVERRIDES: &[(&str, &str, &str, &str)] = &[
    (
        "gpt-5.6-sol",
        "https://developers.openai.com/api/docs/pricing",
        "2026-11-19",
        "precio promocional hasta 2026-11-30 (otra fuente dice 2026-11-21); luego 8/40",
    ),
    (
        "gemini-3-pro",
        "https://ai.google.dev/gemini-api/docs/pricing",
        "2026-11-19",
        "equivale a gemini-3.1-pro-preview: $2/$12 hasta 200k de entrada, $4/$18 por encima",
    ),
    (
        "gemini-3.8-flash",
        "https://ai.google.dev/gemini-api/docs/pricing",
        "2026-11-19",
        "precio promo hasta 2026-12-31 ($0.75/$3.75); desde 2027-01-01 sube a $1.50/$7.50",
    ),
    (
        "minimax-m3",
        "https://platform.minimax.io/docs/pricing/overview",
        "2026-11-19",
        "tarifa corta (contexto <=512k) con 50% de descuento permanente; por encima cuesta el doble",
    ),
    (
        "minimax-m3.1-flash",
        "https://platform.minimax.io/docs/pricing/overview",
        "2026-11-19",
        "tarifa corta (contexto <=512k); por encima cuesta el doble",
    ),
    (
        "deepseek-v4-pro",
        "https://api-docs.deepseek.com/quick_start/pricing",
        "2026-11-19",
        "tarifa de pico (conservadora a propósito); en valle es la mitad",
    ),
    (
        "deepseek-flash",
        "https://api-docs.deepseek.com/quick_start/pricing",
        "2026-11-19",
        "tarifa de pico (conservadora a propósito); en valle es la mitad",
    ),
    (
        "kimi-k3.1-pro",
        "https://platform.kimi.ai/docs/pricing",
        "2026-11-19",
        "tarifa directa de Moonshot; OpenRouter lista otra (0.24/8.40 al 2026-11-16) por ser otra ruta",
    ),
];

/// Procedencia de un precio: cuánto cuesta, de dónde se copió y cuándo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PriceProvenance {
    /// Modelo tal y como aparece en [`PRICING_TABLE`].
    pub model: String,
    /// Proveedor dueño del modelo.
    pub provider: String,
    /// USD por millón de tokens de entrada.
    pub input_usd_per_mtok: f64,
    /// USD por millón de tokens de salida.
    pub output_usd_per_mtok: f64,
    /// URL oficial donde se leyó el precio.
    pub source: String,
    /// Fecha (YYYY-MM-DD) en que se verificó el precio.
    pub verified_on: String,
    /// Nota del caso particular (promociones, tarifas alternativas).
    pub note: String,
}

/// Procedencia del precio de un modelo (precio + fuente + fecha).
///
/// `None` si el modelo no está en [`PRICING_TABLE`] **o** si su proveedor no
/// tiene fuente declarada: en ese caso el precio no es auditable y el motor
/// lo trata como desconocido (no como gratis).
pub fn price_provenance(model: &str) -> Option<PriceProvenance> {
    let m = normalize_model(model);
    let (name, inp, out, prov) = lookup_price_entry(&m)?;

    for (om, url, date, note) in PRICING_OVERRIDES {
        if m.starts_with(om) {
            return Some(PriceProvenance {
                model: name.to_string(),
                provider: prov.to_string(),
                input_usd_per_mtok: inp,
                output_usd_per_mtok: out,
                source: url.to_string(),
                verified_on: date.to_string(),
                note: note.to_string(),
            });
        }
    }

    for (p, url, date) in PRICING_SOURCES {
        if *p == prov {
            return Some(PriceProvenance {
                model: name.to_string(),
                provider: prov.to_string(),
                input_usd_per_mtok: inp,
                output_usd_per_mtok: out,
                source: url.to_string(),
                verified_on: date.to_string(),
                note: String::new(),
            });
        }
    }

    None
}

/// Libro de precios: una entrada por modelo de la tabla, con su procedencia.
pub fn pricing_ledger() -> Vec<PriceProvenance> {
    PRICING_TABLE
        .iter()
        .filter_map(|(name, inp, out, prov)| {
            let mut entry = price_provenance(name)?;
            // `price_provenance` devuelve el nombre canónico de la tabla;
            // aquí se conserva el nombre exacto de la entrada.
            entry.model = name.to_string();
            entry.provider = prov.to_string();
            entry.input_usd_per_mtok = *inp;
            entry.output_usd_per_mtok = *out;
            Some(entry)
        })
        .collect()
}

/// Precios cuya verificación es más antigua que `max_age_days`.
///
/// Son los que hay que **re-verificar** antes de fiarse del presupuesto: un
/// precio rancio no se corrige a ojo, se vuelve a leer en la fuente.
pub fn pricing_stale(max_age_days: u64, today_epoch_day: u64) -> Vec<PriceProvenance> {
    pricing_ledger()
        .into_iter()
        .filter(|p| days_since(&p.verified_on, today_epoch_day).map(|d| d > max_age_days).unwrap_or(true))
        .collect()
}

/// Informe JSON de precios: libro completo + los que están rancios.
///
/// Pensado para el cockpit y para el informe de 3 líneas: cualquier consumidor
/// puede ver de dónde sale cada número y cuál hay que re-verificar.
pub fn pricing_report(max_age_days: u64) -> Value {
    let today = today_epoch_day();
    let ledger = pricing_ledger();
    let stale = pricing_stale(max_age_days, today);
    json!({
        "verified_on": PRICING_VERIFIED_ON,
        "today": today,
        "max_age_days": max_age_days,
        "models": ledger.len(),
        "stale_models": stale.len(),
        "ledger": ledger,
        "stale": stale,
    })
}

/// Días transcurridos desde la fecha ISO `YYYY-MM-DD` hasta `today_epoch_day`.
///
/// `None` si la fecha no es válida.  Si la fecha es futura devuelve `0`.
pub fn days_since(iso: &str, today_epoch_day: u64) -> Option<u64> {
    let d = parse_iso_date(iso)?;
    let today = today as i64;
    if today <= d {
        return Some(0);
    }
    Some((today - d) as u64)
}

/// Día epoch (días desde 1970-01-01) de hoy, en UTC.
pub fn today_epoch_day() -> u64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs / 86_400
}

/// Parsea `YYYY-MM-DD` a día epoch.
fn parse_iso_date(s: &str) -> Option<u64> {
    let p: Vec<&str> = s.trim().split('-').collect();
    if p.len() != 3 {
        return None;
    }
    let y: i64 = p[0].parse().ok()?;
    let m: i64 = p[1].parse().ok()?;
    let d: i64 = p[2].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || y < 1970 {
        return None;
    }
    Some(days_from_civil(y, m, d) as u64)
}

/// Días desde 1970-01-01 para una fecha proleptica gregoriana.
///
/// Algoritmo de días civiles (Hinnant): aritmética entera, sin dependencias
/// nuevas y sin zona horaria (fechas, no instantes).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Marcas que identifican un modelo **gratuito** (coste 0) o local.
pub const FREE_MODEL_MARKERS: &[&str] = &[
    "free",       // cualquier "…-free", "…:free", "free-tier"
    ":free",
    "-free",
    "local",      // SEERE / ollama / lmstudio → offline
    "offline",
    "ollama",
    "llama.cpp",
    "lmstudio",
    "pollinations",
    "ovhcloud",
    "gpt-oss",    // Groq free tier
    "glm-4.7-flash", // Z.ai free
    "qwen3-",     // Groq free (Qwen3)
    "openrouter/", // OpenRouter :free
];

/// Normaliza el nombre de un modelo (minúsculas, sin espacios).
pub fn normalize_model(model: &str) -> String {
    model.trim().to_ascii_lowercase()
}

/// ¿Este modelo es gratuito (o local) y por tanto cuesta 0?
pub fn is_free_model(model: &str) -> bool {
    let m = normalize_model(model);
    if m.is_empty() {
        // Sin modelo no se puede afirmar que sea gratis: se trata como
        // desconocido (estimación conservadora), no como gratis.
        return false;
    }
    FREE_MODEL_MARKERS.iter().any(|k| m.contains(k))
}

/// Precio de lista por millón de tokens: `(entrada, salida)` en USD.
/// `None` si el modelo no está en la tabla.
pub fn model_price(model: &str) -> Option<(f64, f64)> {
    let m = normalize_model(model);
    if let Some(p) = lookup_price(&m) {
        return Some(p);
    }
    // Modelos con prefijo de vendor: "openai/gpt-5" → "gpt-5".
    if let Some(idx) = m.rfind('/') {
        let suffix = &m[idx + 1..];
        if let Some(p) = lookup_price(suffix) {
            return Some(p);
        }
    }
    None
}

fn lookup_price(m: &str) -> Option<(f64, f64)> {
    lookup_price_entry(m).map(|(_, i, o, _)| (i, o))
}

/// Busca la entrada de la tabla que corresponde a un modelo.
///
/// Devuelve `(nombre_en_la_tabla, entrada, salida, proveedor)`.  Prefiere la
/// entrada **más larga** que case (p. ej. `claude-opus-4.5` gana a
/// `claude-opus-4`) y resuelve el prefijo de vendedor
/// (`openai/gpt-5` → `gpt-5`).
fn lookup_price_entry(m: &str) -> Option<(&'static str, f64, f64, &'static str)> {
    let mut best: Option<(&'static str, f64, f64, &'static str)> = None;
    for (name, inp, out, prov) in PRICING_TABLE {
        if m.starts_with(name) {
            let better = match best {
                None => true,
                Some((bn, _, _, _)) => name.len() > bn.len(),
            };
            if better {
                best = Some((name, *inp, *out, prov));
            }
        }
    }
    if best.is_some() {
        return best;
    }
    // Prefijo de vendedor: "openai/gpt-5" → "gpt-5".
    if let Some(idx) = m.rfind('/') {
        return lookup_price_entry(&m[idx + 1..]);
    }
    None
}

/// Coste estimado en USD de una llamada con `input_tokens` de entrada y
/// `output_tokens` de salida.
///
/// * Los modelos **gratuitos** (tier free, modelos locales, `:free`) cuestan
///   **0.0**.
/// * Los modelos conocidos usan la tabla de precios de lista.
/// * Los modelos **desconocidos** se estiman con una tarifa conservadora
///   por defecto (1.25 $/MTok entrada, 10 $/MTok salida, escala de `gpt-5`)
///   para no subestimar el gasto: [`model_price`] permite al llamante
///   detectar el caso "no tarifado".
pub fn estimate_cost_usd(model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
    if is_free_model(model) {
        return 0.0;
    }
    let (i_rate, o_rate) = model_price(model).unwrap_or((1.25, 10.00));
    (input_tokens as f64 / 1_000_000.0) * i_rate + (output_tokens as f64 / 1_000_000.0) * o_rate
}

// ─────────────────────────────────────────────────────────────────────────────
// Plantillas de prompt
// ─────────────────────────────────────────────────────────────────────────────

/// Plantillas de prompt por tipo de tarea.  `{prompt}` es el hueco donde va
/// el encargo concreto.
pub const PROMPT_TEMPLATES: &[(&str, &str)] = &[
    (
        "research",
        "ROL: analista de investigación.\n\
         ENCARGO: {prompt}\n\n\
         REQUISITOS:\n\
         1. Investiga a fondo y contrasta varias fuentes.\n\
         2. Cita cada fuente con URL y fecha de publicación.\n\
         3. Separa explícitamente HECHOS de INFERENCIAS.\n\
         4. Cierra con las incertidumbres que no has podido resolver.\n\
         FORMATO: JSON con las claves resumen, hallazgos[], fuentes[], incertidumbres[].\n\
         PROHIBIDO inventar referencias o datos.",
    ),
    (
        "writing",
        "ROL: redactor.\n\
         ENCARGO: {prompt}\n\n\
         REQUISITOS:\n\
         1. Texto claro, directo y sin relleno; sin muletillas.\n\
         2. Adapta el registro al lector que se indica en el encargo.\n\
         3. Si el encargo es ambiguo, elige la interpretación más razonable y dilo.\n\
         4. Sin datos inventados ni cifras sin fuente.\n\
         FORMATO: 3 variantes breves + la versión final recomendada + notas de cambios.",
    ),
    (
        "code",
        "ROL: ingeniero de software.\n\
         ENCARGO: {prompt}\n\n\
         REQUISITOS:\n\
         1. Devuelve el código completo y ejecutable, sin placeholders ni pseudocódigo.\n\
         2. Explica las decisiones no obvias.\n\
         3. Señala riesgos, casos límite y cómo probarlo.\n\
         4. Mantén el estilo del repositorio si se describe.\n\
         FORMATO: código + resumen de cambios + lista de pruebas sugeridas.",
    ),
    (
        "summary",
        "ROL: resumidor.\n\
         ENCARGO: {prompt}\n\n\
         REQUISITOS:\n\
         1. Resume sin perder decisiones, cifras ni nombres propios.\n\
         2. No añadas nada que no esté en el original.\n\
         FORMATO: resumen ejecutivo (5 líneas) + puntos clave en viñetas + decisiones + próximos pasos.",
    ),
    (
        "audit",
        "ROL: auditor de código/seguridad.\n\
         ENCARGO: {prompt}\n\n\
         REQUISITOS:\n\
         1. Cada hallazgo con severidad (ALTA/MEDIA/BAJA), evidencia (fichero:línea) e impacto.\n\
         2. Propón la corrección concreta de cada hallazgo.\n\
         3. Marca lo que no se puede verificar con el material dado.\n\
         4. No inventes vulnerabilidades: si no hay defecto, dilo.\n\
         FORMATO: lista de hallazgos + veredicto final.",
    ),
    (
        "classification",
        "ROL: clasificador.\n\
         ENCARGO: {prompt}\n\n\
         REQUISITOS:\n\
         1. Usa sólo las etiquetas canónicas indicadas en el encargo.\n\
         2. Devuelve una sola etiqueta aunque la confianza sea baja.\n\
         3. Razona en una frase.\n\
         FORMATO: JSON estricto {\"label\": \"...\", \"confidence\": 0.0-1.0, \"reason\": \"...\"}.",
    ),
];

/// Normaliza el nombre de un tipo de tarea (minúsculas, sin acentos) y lo
/// mapea a una clave canónica de plantilla.
fn canonical_kind(kind: &str) -> Option<&'static str> {
    let k = kind
        .trim()
        .to_lowercase()
        .replace('ó', "o")
        .replace('Ó', "o")
        .replace('í', "i")
        .replace('Í', "i")
        .replace('á', "a")
        .replace('Á', "a")
        .replace('é', "e")
        .replace('É', "e")
        .replace('ú', "u")
        .replace('Ú', "u")
        .replace('ñ', "n");
    let k = k.replace([' ', '-'], "_");
    Some(match k.as_str() {
        "research" | "investigacion" | "busqueda" | "research_web" => "research",
        "writing" | "redaccion" | "draft" | "copywriting" | "texto" => "writing",
        "code" | "codigo" | "programacion" | "programming" | "dev" => "code",
        "summary" | "resumen" | "resumir" | "tldr" => "summary",
        "audit" | "auditoria" | "review" | "revision" | "code_review" => "audit",
        "classification" | "clasificacion" | "label" | "etiquetado" | "clase" => "classification",
        _ => return None,
    })
}

/// Plantilla de prompt para un tipo de tarea (`research`, `writing`, `code`,
/// `summary`, `audit`, `classification`; se aceptan alias en español).
/// `None` si el tipo no tiene plantilla.
pub fn prompt_template(kind: &str) -> Option<&'static str> {
    let key = canonical_kind(kind)?;
    PROMPT_TEMPLATES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, t)| *t)
}

/// Renderiza la plantilla de un tipo de tarea con el encargo dentro.
pub fn render_prompt(kind: &str, prompt: &str) -> Option<String> {
    let t = prompt_template(kind)?;
    if t.contains("{prompt}") {
        Some(t.replace("{prompt}", prompt))
    } else {
        Some(format!("{}\n\n{}", t, prompt))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Utilidades
// ─────────────────────────────────────────────────────────────────────────────

/// ¿Este tier de cuenta no cuesta dinero (cuota ya pagada)?
pub fn tier_is_free(tier: &str) -> bool {
    matches!(
        tier.trim().to_lowercase().as_str(),
        "free" | "trial" | "family" | "subscription" | "local" | "keyless" | "offline"
    )
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn norm_cap(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Compara capacidades sin distinguir mayúsculas/espacios.
pub fn has_capabilities(account: &ProviderAccount, task: &DelegationTask) -> bool {
    task.required_capabilities
        .iter()
        .all(|need| account.capabilities.iter().any(|c| norm_cap(c) == norm_cap(need)))
}

/// Estimación de tokens de una tarea: entrada a partir del prompt y salida a
/// partir de `max_tokens`.
pub fn estimate_task_tokens(task: &DelegationTask) -> (u32, u32) {
    let prompt_tokens = (task.prompt.chars().count() as u32 / 4).saturating_add(64);
    let out = if task.max_tokens == 0 {
        default_max_tokens()
    } else {
        task.max_tokens
    };
    (prompt_tokens, out)
}

// ─────────────────────────────────────────────────────────────────────────────
// El motor
// ─────────────────────────────────────────────────────────────────────────────

/// Motor de delegación: cola + enrutado + contabilidad + auto-mejora.
#[derive(Clone, Debug, Default)]
pub struct DelegateEngine {
    accounts: Vec<ProviderAccount>,
    tasks: Vec<DelegationTask>,
    learning: HashMap<String, AccountLearning>,
    seq: u64,
}

impl DelegateEngine {
    /// Crea el motor con una lista de cuentas.  Las cuentas con `id` vacío se
    /// descartan y los `id` duplicados se colapsan (gana la primera).
    pub fn new(accounts: Vec<ProviderAccount>) -> Self {
        let mut accs: Vec<ProviderAccount> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for a in accounts {
            if a.id.trim().is_empty() {
                continue;
            }
            if !seen.insert(a.id.clone()) {
                continue; // duplicado: gana la primera
            }
            accs.push(a);
        }
        let mut learning = HashMap::new();
        for a in &accs {
            learning.entry(a.id.clone()).or_default();
        }
        DelegateEngine {
            accounts: accs,
            tasks: Vec::new(),
            learning,
            seq: 0,
        }
    }

    /// Cuentas registradas (solo lectura).
    pub fn accounts(&self) -> &[ProviderAccount] {
        &self.accounts
    }

    /// Cola de tareas (solo lectura).
    pub fn tasks(&self) -> &[DelegationTask] {
        &self.tasks
    }

    /// Una cuenta por id.
    pub fn account(&self, id: &str) -> Option<&ProviderAccount> {
        self.accounts.iter().find(|a| a.id == id)
    }

    /// Una tarea por id.
    pub fn task(&self, id: &str) -> Option<&DelegationTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Tareas que aún esperan ejecución.
    pub fn queue(&self) -> Vec<&DelegationTask> {
        self.tasks
            .iter()
            .filter(|t| is_plannable(&t.status))
            .collect()
    }

    /// Encola una tarea y devuelve su id.  Si la tarea no trae id, se genera
    /// uno; el estado por defecto es `queued` y se anota la cuenta que el
    /// enrutado elegiría ahora mismo (puede cambiar en el plan final).
    pub fn submit(&mut self, mut task: DelegationTask) -> String {
        if task.id.trim().is_empty() {
            self.seq += 1;
            task.id = format!("task-{}-{:04}", now_epoch(), self.seq);
        }
        if task.kind.trim().is_empty() {
            task.kind = "generic".into();
        }
        if task.status.trim().is_empty() {
            task.status = "queued".into();
        }
        if task.max_tokens == 0 {
            task.max_tokens = default_max_tokens();
        }
        task.assigned_account = self.pick_account(&task).ok();
        let id = task.id.clone();
        match self.tasks.iter_mut().find(|t| t.id == id) {
            Some(slot) => *slot = task,
            None => self.tasks.push(task),
        }
        id
    }

    /// Elige cuenta para una tarea.
    ///
    /// Heurística **determinista y pura** (no muta nada).  Es una suma
    /// ponderada cuyos pesos reproducen este orden de preferencia:
    ///
    /// 1. sólo cuentas habilitadas, con cuota no agotada y con **todas** las
    ///    capacidades requeridas;
    /// 2. las cuentas **gratuitas** ganan siempre a las de pago (+1000): las
    ///    cuentas de pago quedan reservadas como último recurso;
    /// 3. a igualdad de tier, más **cuota restante** es mejor;
    /// 4. a igualdad de cuota, **menor coste estimado** es mejor;
    /// 5. las cuentas que fallaron **recientemente** (fallos consecutivos) y
    ///    las que tienen peor tasa de éxito global son penalizadas;
    /// 6. desempate por **calidad aprendida** (EWMA) y por **prioridad**
    ///    declarada; si todo empata, por `id` (alfabético), de modo que el
    ///    resultado es siempre el mismo para la misma entrada.
    pub fn pick_account(&self, task: &DelegationTask) -> Result<String, String> {
        let mut best: Option<(&ProviderAccount, f64)> = None;
        let mut blocked_capability = false;
        let mut blocked_quota = false;
        let mut blocked_disabled = false;

        for acc in &self.accounts {
            if !acc.enabled {
                blocked_disabled = true;
                continue;
            }
            if acc.remaining_quota == Some(0) {
                blocked_quota = true; // cuenta quemada: no recibe nada
                continue;
            }
            if !has_capabilities(acc, task) {
                blocked_capability = true;
                continue;
            }
            let sc = self.routing_score(acc, task);
            match best {
                None => best = Some((acc, sc)),
                Some((b, bs)) => {
                    let better = sc > bs + 1e-9
                        || ((sc - bs).abs() <= 1e-9 && acc.id.as_str() < b.id.as_str());
                    if better {
                        best = Some((acc, sc));
                    }
                }
            }
        }

        match best {
            Some((acc, _)) => Ok(acc.id.clone()),
            None => {
                if self.accounts.is_empty() {
                    Err("no hay cuentas registradas".into())
                } else if blocked_capability {
                    Err(format!(
                        "ninguna cuenta habilitada tiene las capacidades requeridas: {:?}",
                        task.required_capabilities
                    ))
                } else if blocked_quota {
                    Err("todas las cuentas candidatas han agotado la cuota".into())
                } else if blocked_disabled {
                    Err("todas las cuentas están deshabilitadas".into())
                } else {
                    Err("no hay ninguna cuenta capaz de atender la tarea".into())
                }
            }
        }
    }

    /// Puntuación de enrutado (mayor = mejor).  Es la suma ponderada de los
    /// criterios del enrutado; ver [`DelegateEngine::pick_account`].
    fn routing_score(&self, acc: &ProviderAccount, task: &DelegationTask) -> f64 {
        let mut s = 0.0;

        // 1) capa gratuita primero (las de pago son último recurso).
        s += if tier_is_free(&acc.tier) { 1000.0 } else { -1000.0 };

        // 2) cuota restante: más es mejor.  `None` = cuota no medida: se le
        //    supone una capacidad mediana (100), por debajo de una cuenta con
        //    cuota medida abundante y por encima de una con cuota medida corta.
        s += match acc.remaining_quota {
            Some(q) => (q as f64).min(1000.0),
            None => 100.0,
        };

        // 3) coste estimado: menos es mejor (USD → 200 puntos por dólar).
        s -= self.effective_cost(acc, task) * 200.0;

        // 4) calidad aprendida (EWMA 0..1 → hasta 25 puntos).
        s += self.score(&acc.id) as f64 * 25.0;

        // 5) penalización por fallos recientes y por mal historial.
        let l = self.learning.get(&acc.id);
        let consec = l.map(|x| x.consecutive_failures).unwrap_or(0);
        let rate = l
            .map(|x| if x.attempts == 0 { 0.0 } else { x.failed as f64 / x.attempts as f64 })
            .unwrap_or(0.0);
        s -= consec as f64 * 30.0;
        s -= rate * 15.0;

        // 6) prioridad declarada por el usuario.
        s += acc.priority as f64;

        s
    }

    /// Coste efectivo de una tarea en una cuenta: 0 USD si la cuenta es
    /// gratuita, el precio de lista si es de pago.
    fn effective_cost(&self, acc: &ProviderAccount, task: &DelegationTask) -> f64 {
        let (i, o) = estimate_task_tokens(task);
        if tier_is_free(&acc.tier) {
            0.0
        } else {
            estimate_cost_usd(&acc.model, i, o)
        }
    }

    /// Registra el resultado de un intento: actualiza la tarea, el
    /// aprendizaje de la cuenta, los tokens, el coste y consume una unidad de
    /// cuota.  Si la tarea no existe, el aprendizaje de la cuenta se guarda
    /// igualmente (no se pierde la señal).
    pub fn record_attempt(&mut self, task_id: &str, attempt: Attempt) {
        let ok = attempt.ok;
        let account = attempt.account.clone();
        let reward = attempt
            .quality_score
            .unwrap_or(if ok { 1.0 } else { 0.0 })
            .clamp(0.0, 1.0);
        let list_cost = estimate_cost_usd(&attempt.model, attempt.input_tokens, attempt.output_tokens);
        let free = self
            .account(&account)
            .map(|a| tier_is_free(&a.tier))
            .unwrap_or(false);
        let eff_cost = if free { 0.0 } else { list_cost };

        {
            let l = self.learning.entry(account.clone()).or_default();
            l.attempts += 1;
            if ok {
                l.ok += 1;
                l.consecutive_failures = 0;
            } else {
                l.failed += 1;
                l.consecutive_failures += 1;
            }
            l.ewma = if l.attempts == 1 {
                QUALITY_PRIOR + EWMA_ALPHA * (reward - QUALITY_PRIOR)
            } else {
                l.ewma + EWMA_ALPHA * (reward - l.ewma)
            };
            l.input_tokens += attempt.input_tokens as u64;
            l.output_tokens += attempt.output_tokens as u64;
            l.cost_usd += eff_cost;
            l.shadow_cost_usd += list_cost;
        }

        // Consumir cuota: un intento es una petición de la cuenta.
        if let Some(acc) = self.accounts.iter_mut().find(|a| a.id == account) {
            if let Some(q) = acc.remaining_quota.as_mut() {
                if *q > 0 {
                    *q -= 1;
                }
            }
        }

        // Adjuntar a la tarea y actualizar su estado.
        let assigned = Some(account);
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            t.assigned_account = assigned;
            t.attempts.push(attempt);
            t.status = if ok {
                "done".into()
            } else if t.attempts.len() < MAX_ATTEMPTS {
                "retry".into()
            } else {
                "failed".into()
            };
        }
    }

    /// EWMA de calidad/éxito de una cuenta (0.0..=1.0).  Una cuenta sin
    /// historial vale [`QUALITY_PRIOR`].
    pub fn score(&self, account: &str) -> f32 {
        match self.learning.get(account) {
            Some(l) if l.attempts > 0 => l.ewma,
            _ => QUALITY_PRIOR,
        }
    }

    /// Estadísticas por cuenta.
    pub fn account_stats(&self) -> Vec<AccountStats> {
        self.accounts
            .iter()
            .map(|a| {
                let l = self.learning.get(&a.id);
                let attempts = l.map(|x| x.attempts).unwrap_or(0);
                let ok = l.map(|x| x.ok).unwrap_or(0);
                let failed = l.map(|x| x.failed).unwrap_or(0);
                AccountStats {
                    account: a.id.clone(),
                    provider: a.provider.clone(),
                    tier: a.tier.clone(),
                    model: a.model.clone(),
                    capabilities: a.capabilities.clone(),
                    priority: a.priority,
                    enabled: a.enabled,
                    attempts,
                    ok,
                    failed,
                    consecutive_failures: l.map(|x| x.consecutive_failures).unwrap_or(0),
                    score: self.score(&a.id),
                    success_rate: if attempts == 0 {
                        0.0
                    } else {
                        ok as f32 / attempts as f32
                    },
                    input_tokens: l.map(|x| x.input_tokens).unwrap_or(0),
                    output_tokens: l.map(|x| x.output_tokens).unwrap_or(0),
                    cost_usd: l.map(|x| x.cost_usd).unwrap_or(0.0),
                    shadow_cost_usd: l.map(|x| x.shadow_cost_usd).unwrap_or(0.0),
                    remaining_quota: a.remaining_quota,
                    daily_budget_usd: a.daily_budget_usd,
                    burned: a.remaining_quota == Some(0),
                }
            })
            .collect()
    }

    /// Plan de ejecución de la cola: qué tarea va a qué cuenta, en qué orden
    /// y por qué.  Orden: primero las tareas con deadline (la más próxima
    /// primero) y después las que no lo tienen, por orden de entrada.
    pub fn plan(&self) -> Vec<PlanStep> {
        let mut idx: Vec<usize> = (0..self.tasks.len())
            .filter(|i| is_plannable(&self.tasks[*i].status))
            .collect();
        idx.sort_by(|a, b| {
            let ta = &self.tasks[*a];
            let tb = &self.tasks[*b];
            deadline_key(ta)
                .cmp(&deadline_key(tb))
                .then(a.cmp(b))
        });

        let mut steps = Vec::with_capacity(idx.len());
        for (n, i) in idx.into_iter().enumerate() {
            let task = &self.tasks[i];
            let (in_tok, out_tok) = estimate_task_tokens(task);
            match self.pick_account(task) {
                Ok(id) => {
                    let acc = self
                        .account(&id)
                        .expect("la cuenta devuelta existe");
                    let cost = self.effective_cost(acc, task);
                    steps.push(PlanStep {
                        order: n + 1,
                        task_id: task.id.clone(),
                        kind: task.kind.clone(),
                        account: acc.id.clone(),
                        model: acc.model.clone(),
                        estimated_input_tokens: in_tok,
                        estimated_output_tokens: out_tok,
                        estimated_cost_usd: cost,
                        reason: self.routing_reason(acc, task, cost),
                    });
                }
                Err(e) => steps.push(PlanStep {
                    order: n + 1,
                    task_id: task.id.clone(),
                    kind: task.kind.clone(),
                    account: String::new(),
                    model: String::new(),
                    estimated_input_tokens: in_tok,
                    estimated_output_tokens: out_tok,
                    estimated_cost_usd: 0.0,
                    reason: format!("SIN CUENTA: {}", e),
                }),
            }
        }
        steps
    }

    /// Razón legible de por qué se eligió una cuenta.
    fn routing_reason(&self, acc: &ProviderAccount, task: &DelegationTask, cost: f64) -> String {
        let free = tier_is_free(&acc.tier);
        let quota = match acc.remaining_quota {
            Some(q) => format!("cuota {}", q),
            None => "cuota sin medir".to_string(),
        };
        let list = estimate_cost_usd(
            &acc.model,
            estimate_task_tokens(task).0,
            estimate_task_tokens(task).1,
        );
        format!(
            "cuenta {} ({} · {}) con {}; coste efectivo ${:.4} (lista ${:.4}); calidad {:.2}; prioridad {}",
            acc.id,
            acc.provider,
            acc.tier,
            quota,
            cost,
            list,
            self.score(&acc.id),
            acc.priority
        ) + if free {
            "; gratuita: se reserva la de pago"
        } else {
            "; de pago: sólo porque no hay alternativa gratuita"
        }
    }

    /// Estadísticas globales: tokens gastados, coste estimado, tasa de éxito
    /// por cuenta y cuentas quemadas.
    pub fn stats(&self) -> Value {
        let st = self.account_stats();
        let mut input_tokens = 0u64;
        let mut output_tokens = 0u64;
        let mut cost = 0.0f64;
        let mut shadow = 0.0f64;
        let mut attempts = 0u64;
        let mut ok = 0u64;
        let mut by_account = serde_json::Map::new();
        let mut burned: Vec<String> = Vec::new();
        let mut disabled: Vec<String> = Vec::new();
        let mut free_accounts = 0u64;
        let mut paid_accounts = 0u64;

        for a in &st {
            input_tokens += a.input_tokens;
            output_tokens += a.output_tokens;
            cost += a.cost_usd;
            shadow += a.shadow_cost_usd;
            attempts += a.attempts as u64;
            ok += a.ok as u64;
            if a.burned {
                burned.push(a.account.clone());
            }
            if !a.enabled {
                disabled.push(a.account.clone());
            }
            if tier_is_free(&a.tier) {
                free_accounts += 1;
            } else {
                paid_accounts += 1;
            }
            by_account.insert(
                a.account.clone(),
                json!({
                    "provider": a.provider,
                    "tier": a.tier,
                    "model": a.model,
                    "attempts": a.attempts,
                    "ok": a.ok,
                    "failed": a.failed,
                    "success_rate": a.success_rate,
                    "score": a.score,
                    "input_tokens": a.input_tokens,
                    "output_tokens": a.output_tokens,
                    "cost_usd": a.cost_usd,
                    "shadow_cost_usd": a.shadow_cost_usd,
                    "remaining_quota": a.remaining_quota,
                    "burned": a.burned,
                }),
            );
        }

        let mut task_counts: HashMap<&str, u64> = HashMap::new();
        for t in &self.tasks {
            *task_counts.entry(status_bucket(&t.status)).or_insert(0) += 1;
        }
        let get = |k: &str| *task_counts.get(k).unwrap_or(&0);

        json!({
            "tasks": {
                "total": self.tasks.len(),
                "queued": get("queued"),
                "retry": get("retry"),
                "done": get("done"),
                "failed": get("failed"),
            },
            "tokens": {
                "input": input_tokens,
                "output": output_tokens,
                "total": input_tokens + output_tokens,
            },
            "cost_usd": {
                "estimated": cost,
                "shadow_if_paid": shadow,
                "savings": shadow - cost,
            },
            "attempts": {
                "total": attempts,
                "ok": ok,
                "failed": attempts.saturating_sub(ok),
            },
            "success_rate": {
                "overall": if attempts == 0 { 0.0 } else { ok as f64 / attempts as f64 },
                "by_account": Value::Object(by_account),
            },
            "accounts": {
                "total": st.len(),
                "free": free_accounts,
                "paid": paid_accounts,
                "burned": burned,
                "disabled": disabled,
            },
            "queue_depth": self.queue().len(),
        })
    }

    /// Recomendaciones accionables derivadas de las estadísticas.  Es el
    /// "qué corregir" del bucle *delegar → medir → corregir → delegar*.
    pub fn adjustments(&self) -> Vec<Adjustment> {
        let st = self.account_stats();
        let mut out: Vec<Adjustment> = Vec::new();

        // 1) Cuentas quemadas: apagarlas hasta el reset de cuota.
        for a in st.iter().filter(|a| a.burned) {
            out.push(Adjustment {
                account: a.account.clone(),
                action: AdjustmentAction::Disable,
                reason: format!(
                    "cuota agotada tras {} intentos: desactivar hasta el reset de la cuota",
                    a.attempts
                ),
            });
        }

        // 2) Cuentas gratuitas oídas sin uso: activarlas para repartir carga.
        for a in st
            .iter()
            .filter(|a| !a.enabled && tier_is_free(&a.tier) && a.attempts == 0 && !a.burned)
        {
            out.push(Adjustment {
                account: a.account.clone(),
                action: AdjustmentAction::Enable,
                reason: "cuenta gratuita sin uso: activarla para repartir la carga y ahorrar tokens"
                    .into(),
            });
        }

        // 3) Mal historial: bajar prioridad.
        for a in st
            .iter()
            .filter(|a| a.attempts >= 3 && a.success_rate < 0.5 && !a.burned)
        {
            let p = (a.priority - 15).max(-MAX_PRIORITY);
            out.push(Adjustment {
                account: a.account.clone(),
                action: AdjustmentAction::SetPriority(p),
                reason: format!(
                    "tasa de éxito {:.0}% tras {} intentos: bajar prioridad a {}",
                    a.success_rate * 100.0,
                    a.attempts,
                    p
                ),
            });
        }

        // 4) Buen historial y cuota: subir prioridad (rebalanceo).
        for a in st
            .iter()
            .filter(|a| a.attempts >= 3 && a.success_rate >= 0.9 && !a.burned)
        {
            let p = (a.priority + 10).min(MAX_PRIORITY);
            out.push(Adjustment {
                account: a.account.clone(),
                action: AdjustmentAction::SetPriority(p),
                reason: format!(
                    "tasa de éxito {:.0}% tras {} intentos con cuota sana: subir prioridad a {}",
                    a.success_rate * 100.0,
                    a.attempts,
                    p
                ),
            });
        }

        // 5) Presupuesto diario excedido: bajarlo.
        for a in st
            .iter()
            .filter(|a| a.daily_budget_usd > 0.0 && a.cost_usd > a.daily_budget_usd)
        {
            let b = (a.daily_budget_usd * 0.5).max(0.0);
            out.push(Adjustment {
                account: a.account.clone(),
                action: AdjustmentAction::SetBudgetUsd(b),
                reason: format!(
                    "gasto ${:.4} superior al presupuesto diario ${:.4}: bajar el presupuesto a ${:.4}",
                    a.cost_usd, a.daily_budget_usd, b
                ),
            });
        }

        // 6) Rebalanceo: si hay una cuenta gratuita con las mismas
        //    capacidades que una de pago que ya se ha usado, relegar la de pago.
        for a in st.iter().filter(|a| !tier_is_free(&a.tier) && a.attempts > 0) {
            let free_alt = st.iter().any(|f| {
                f.account != a.account
                    && tier_is_free(&f.tier)
                    && f.enabled
                    && !f.burned
                    && f.capabilities.iter().any(|c| {
                        a.capabilities.iter().any(|ac| norm_cap(ac) == norm_cap(c))
                    })
            });
            if free_alt {
                let p = (a.priority - 20).max(-MAX_PRIORITY);
                out.push(Adjustment {
                    account: a.account.clone(),
                    action: AdjustmentAction::SetPriority(p),
                    reason: format!(
                        "hay una cuenta gratuita con las mismas capacidades: relegar la de pago (prioridad {})",
                        p
                    ),
                });
            }
        }

        // Tope de recomendaciones para no ahogar al usuario.
        out.truncate(32);
        out
    }

    /// Aplica las recomendaciones validando cada una.  Es **todo o nada**:
    /// si alguna ajuste es inválido, no se aplica ninguno y se devuelve el
    /// error.
    pub fn apply_adjustments(&mut self, adj: &[Adjustment]) -> Result<(), String> {
        // Fase 1: validar (sin tocar nada).
        for a in adj {
            if !self.accounts.iter().any(|x| x.id == a.account) {
                return Err(format!("cuenta desconocida: {}", a.account));
            }
            match &a.action {
                AdjustmentAction::SetPriority(p) => {
                    if *p < -MAX_PRIORITY || *p > MAX_PRIORITY {
                        return Err(format!(
                            "prioridad fuera de rango ({}): debe estar entre -{} y {}",
                            p, MAX_PRIORITY, MAX_PRIORITY
                        ));
                    }
                }
                AdjustmentAction::SetBudgetUsd(b) => {
                    if !b.is_finite() || *b < 0.0 || *b > MAX_DAILY_BUDGET_USD {
                        return Err(format!(
                            "presupuesto inválido ({}): debe estar entre 0.0 y {}",
                            b, MAX_DAILY_BUDGET_USD
                        ));
                    }
                }
                AdjustmentAction::SetModel(m) => {
                    if m.trim().is_empty() {
                        return Err("modelo vacío".to_string());
                    }
                }
                AdjustmentAction::Enable | AdjustmentAction::Disable => {}
            }
        }
        // Fase 2: aplicar.
        for a in adj {
            if let Some(acc) = self.accounts.iter_mut().find(|x| x.id == a.account) {
                match &a.action {
                    AdjustmentAction::Enable => acc.enabled = true,
                    AdjustmentAction::Disable => acc.enabled = false,
                    AdjustmentAction::SetPriority(p) => acc.priority = *p,
                    AdjustmentAction::SetBudgetUsd(b) => acc.daily_budget_usd = *b,
                    AdjustmentAction::SetModel(m) => acc.model = m.clone(),
                }
            }
        }
        Ok(())
    }
}

fn is_plannable(status: &str) -> bool {
    matches!(
        status.trim().to_lowercase().as_str(),
        "" | "queued" | "pending" | "retry" | "running"
    )
}

fn status_bucket(status: &str) -> &'static str {
    match status.trim().to_lowercase().as_str() {
        "queued" | "pending" | "" => "queued",
        "retry" | "running" => "retry",
        "done" | "ok" | "success" => "done",
        _ => "failed",
    }
}

/// Clave de ordenación por deadline: las tareas con deadline van primero (la
/// más próxima primero); las que no lo tienen, al final.
fn deadline_key(task: &DelegationTask) -> (u8, String) {
    match &task.deadline {
        Some(d) if !d.trim().is_empty() => (0, d.trim().to_string()),
        _ => (1, String::new()),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Importación / exportación JSON de cuentas
// ─────────────────────────────────────────────────────────────────────────────

/// Tiers admitidos al importar cuentas.
pub const VALID_TIERS: &[&str] = &[
    "free",
    "trial",
    "family",
    "subscription",
    "paid",
    "prepaid",
    "local",
    "keyless",
];

/// Exporta las cuentas a JSON (pretty).  Formato:
/// `{"version": 1, "accounts": [ ... ]}`.
pub fn save_accounts(accounts: &[ProviderAccount]) -> String {
    let payload = json!({
        "version": 1,
        "accounts": accounts,
    });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
}

/// Importa cuentas desde JSON con **validación estricta**.
///
/// Acepta un array plano `[ ... ]` o un objeto `{"accounts": [ ... ]}`.
/// Rechaza: campos obligatorios vacíos, `id` duplicados, cuotas negativas o
/// no enteras, presupuestos negativos o por encima de
/// [`MAX_DAILY_BUDGET_USD`], prioridades fuera de rango, `enabled` que no sea
/// booleano y capacidades vacías.
pub fn load_accounts(json_str: &str) -> Result<Vec<ProviderAccount>, String> {
    let root: Value = serde_json::from_str(json_str)
        .map_err(|e| format!("JSON inválido: {}", e))?;
    let arr = match &root {
        Value::Array(a) => a.clone(),
        Value::Object(o) => match o.get("accounts") {
            Some(Value::Array(a)) => a.clone(),
            Some(_) => return Err("\"accounts\" debe ser un array".into()),
            None => return Err("falta el array \"accounts\"".into()),
        },
        _ => return Err("la raíz debe ser un array o un objeto con \"accounts\"".into()),
    };
    if arr.is_empty() {
        return Err("no hay cuentas en el JSON".into());
    }

    let mut out: Vec<ProviderAccount> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for (i, v) in arr.iter().enumerate() {
        let obj = v
            .as_object()
            .ok_or_else(|| format!("cuenta {}: no es un objeto JSON", i))?;

        let id = req_str(obj, "id", i)?;
        if id.len() > 64 {
            return Err(format!("cuenta {}: \"id\" demasiado largo (>64)", i));
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ':')
        {
            return Err(format!(
                "cuenta {}: \"id\" con caracteres no permitidos (solo [A-Za-z0-9._:-])",
                i
            ));
        }
        if !seen.insert(id.clone()) {
            return Err(format!("cuenta {}: \"id\" duplicado ({})", i, id));
        }

        let provider = req_str(obj, "provider", i)?;
        let model = req_str(obj, "model", i)?;

        let tier = match obj.get("tier") {
            Some(Value::String(s)) if !s.trim().is_empty() => s.trim().to_lowercase(),
            Some(Value::String(_)) => {
                return Err(format!("cuenta {}: \"tier\" vacío", i));
            }
            Some(_) => return Err(format!("cuenta {}: \"tier\" debe ser texto", i)),
            None => return Err(format!("cuenta {}: falta el campo obligatorio \"tier\"", i)),
        };
        if !VALID_TIERS.contains(&tier.as_str()) {
            return Err(format!(
                "cuenta {}: \"tier\" inválido ({:?}), válidos: {:?}",
                i, tier, VALID_TIERS
            ));
        }

        let mut capabilities = Vec::new();
        match obj.get("capabilities") {
            Some(Value::Array(a)) => {
                for c in a {
                    let s = c
                        .as_str()
                        .ok_or_else(|| format!("cuenta {}: capacidades no textuales", i))?;
                    if s.trim().is_empty() {
                        return Err(format!("cuenta {}: capacidad vacía", i));
                    }
                    capabilities.push(s.trim().to_lowercase());
                }
            }
            Some(_) => return Err(format!("cuenta {}: \"capabilities\" debe ser un array", i)),
            None => {}
        }

        let remaining_quota = match obj.get("remaining_quota") {
            Some(Value::Null) | None => None,
            Some(Value::Number(n)) => {
                let q = n.as_i64().ok_or_else(|| {
                    format!("cuenta {}: \"remaining_quota\" debe ser un entero", i)
                })?;
                if q < 0 {
                    return Err(format!("cuenta {}: \"remaining_quota\" negativo ({})", i, q));
                }
                Some(q as u32)
            }
            Some(_) => {
                return Err(format!(
                    "cuenta {}: \"remaining_quota\" debe ser un entero o null",
                    i
                ))
            }
        };

        let daily_budget_usd = match obj.get("daily_budget_usd") {
            Some(Value::Null) | None => 0.0,
            Some(Value::Number(n)) => {
                let b = n
                    .as_f64()
                    .ok_or_else(|| format!("cuenta {}: \"daily_budget_usd\" inválido", i))?;
                if !b.is_finite() {
                    return Err(format!("cuenta {}: \"daily_budget_usd\" no finito", i));
                }
                if b < 0.0 {
                    return Err(format!("cuenta {}: presupuesto negativo ({})", i, b));
                }
                if b > MAX_DAILY_BUDGET_USD {
                    return Err(format!(
                        "cuenta {}: presupuesto ${:.2} por encima del techo ${:.2}",
                        i, b, MAX_DAILY_BUDGET_USD
                    ));
                }
                b
            }
            Some(_) => {
                return Err(format!(
                    "cuenta {}: \"daily_budget_usd\" debe ser un número",
                    i
                ))
            }
        };

        let priority = match obj.get("priority") {
            Some(Value::Null) | None => 0,
            Some(Value::Number(n)) => {
                let p = n
                    .as_i64()
                    .ok_or_else(|| format!("cuenta {}: \"priority\" debe ser un entero", i))?;
                if p < -MAX_PRIORITY as i64 || p > MAX_PRIORITY as i64 {
                    return Err(format!(
                        "cuenta {}: prioridad {} fuera de rango ({}..={})",
                        i, p, -MAX_PRIORITY, MAX_PRIORITY
                    ));
                }
                p as i32
            }
            Some(_) => return Err(format!("cuenta {}: \"priority\" debe ser un entero", i)),
        };

        let enabled = match obj.get("enabled") {
            Some(Value::Null) | None => true,
            Some(Value::Bool(b)) => *b,
            Some(_) => return Err(format!("cuenta {}: \"enabled\" debe ser booleano", i)),
        };

        let notes = match obj.get("notes") {
            Some(Value::Null) | None => String::new(),
            Some(Value::String(s)) => s.clone(),
            Some(_) => return Err(format!("cuenta {}: \"notes\" debe ser texto", i)),
        };

        out.push(ProviderAccount {
            id,
            provider,
            tier,
            model,
            capabilities,
            remaining_quota,
            daily_budget_usd,
            priority,
            enabled,
            notes,
        });
    }

    Ok(out)
}

fn req_str(obj: &serde_json::Map<String, Value>, field: &str, idx: usize) -> Result<String, String> {
    match obj.get(field) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        Some(Value::String(_)) => Err(format!("cuenta {}: \"{}\" vacío", idx, field)),
        Some(_) => Err(format!("cuenta {}: \"{}\" debe ser texto", idx, field)),
        None => Err(format!("cuenta {}: falta el campo obligatorio \"{}\"", idx, field)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Inventario real de cuentas (P1.3 + P1.8)
// ─────────────────────────────────────────────────────────────────────────────
//
// El enrutado sólo es tan bueno como el inventario que conoce: sin saber qué
// cuentas gratuitas existen, "enrutar a la cuenta gratuita adecuada" es una
// frase.  Este cargador traduce un inventario **de cuentas** (proveedor, nº de
// cuentas, capacidades) en [`ProviderAccount`]s listas para el motor.
//
// Regla dura: el inventario **nunca** lleva secretos.  Los tokens/cookies se
// quedan en el almacén privado (fuera del repo) y aquí sólo se cuenta cuántas
// cuentas hay de cada proveedor.  Si el inventario trae un campo con pinta de
// credencial, el cargador lo rechaza en vez de propagarlo.

/// Modelo por defecto **gratuito** de cada proveedor conocido.
///
/// Si el modelo no está en esta tabla, el inventario tiene que traerlo
/// explícito: enrutar a ciegas a un proveedor desconocido es exactamente el
/// fallo que este módulo quiere evitar.
pub const DEFAULT_FREE_MODEL: &[(&str, &str)] = &[
    ("kimi", "kimi-k3"),
    ("moonshot", "kimi-k3"),
    ("minimax", "minimax-m3"),
    ("deepseek", "deepseek-chat"),
    ("chatgpt", "gpt-5.6-sol"),
    ("openai", "gpt-5.6-sol"),
    ("gemini", "gemini-3-flash"),
    ("google", "gemini-3-flash"),
    ("claude", "claude-sonnet-5.5"),
    ("anthropic", "claude-sonnet-5.5"),
];

/// Campos cuyo nombre delata una credencial: el inventario no debe traerlos.
pub const SECRET_FIELD_NAMES: &[&str] = &[
    "token",
    "tokens",
    "password",
    "passwd",
    "secret",
    "api_key",
    "apikey",
    "cookie",
    "cookies",
    "authorization",
    "auth",
    "session_key",
    "refresh_token",
    "access_token",
    "bearer",
    "private_key",
    "credential",
    "credentials",
];

/// Un proveedor del inventario y cuántas cuentas suyas hay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccountSeed {
    /// Proveedor (`kimi`, `deepseek`, ...).
    pub provider: String,
    /// Cuántas cuentas hay de este proveedor.
    pub count: u32,
    /// Nivel de las cuentas (`free` por defecto).
    #[serde(default)]
    pub tier: Option<String>,
    /// Modelo concreto (por defecto, el gratuito del proveedor).
    #[serde(default)]
    pub model: Option<String>,
    /// Capacidades que pueden resolver.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Prioridad declarada.
    #[serde(default)]
    pub priority: i32,
    /// Notas libres.
    #[serde(default)]
    pub notes: String,
}

/// Busca el campo con pinta de credencial en un JSON (recursivo).
///
/// Devuelve la ruta del campo (``accounts.0.token``) o `None` si el JSON no
/// lleva ninguna credencial.
pub fn find_secret_field(value: &Value, path: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", path, k)
                };
                let kl = k.to_ascii_lowercase();
                if SECRET_FIELD_NAMES.iter().any(|s| kl == *s || kl.ends_with(s)) {
                    return Some(p);
                }
                if let Some(found) = find_secret_field(v, &p) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                if let Some(found) = find_secret_field(v, &format!("{}[{}]", path, i)) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

/// Traduce semillas de inventario en cuentas de proveedor.
///
/// - `count` cuentas por proveedor, con `id` estable `{provider}-{n}`.
/// - El `tier` por defecto es `free` (a eso apunta el módulo: enrutar a lo
///   gratis y reservar lo de pago).
/// - El modelo por defecto es el gratuito del proveedor; si no se conoce el
///   proveedor, hay que traer el modelo explícito.
pub fn accounts_from_seeds(seeds: &[AccountSeed]) -> Result<Vec<ProviderAccount>, String> {
    let mut out: Vec<ProviderAccount> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for (i, seed) in seeds.iter().enumerate() {
        let provider = seed.provider.trim().to_lowercase();
        if provider.is_empty() {
            return Err(format!("semilla {}: \"provider\" vacío", i));
        }
        if seed.count == 0 {
            return Err(format!(
                "semilla {} ({}): \"count\" debe ser >= 1 (una cuenta con count 0 no enrutaría nada)",
                i, provider
            ));
        }
        if seed.count > 1000 {
            return Err(format!("semilla {} ({}): \"count\" absurdo (>1000)", i, provider));
        }

        let tier = seed
            .tier
            .as_deref()
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "free".to_string());
        if !VALID_TIERS.contains(&tier.as_str()) {
            return Err(format!(
                "semilla {} ({}): \"tier\" inválido ({:?}), válidos: {:?}",
                i, provider, tier, VALID_TIERS
            ));
        }

        let model = match seed.model.as_deref().map(|m| m.trim()).filter(|m| !m.is_empty()) {
            Some(m) => m.to_string(),
            None => match DEFAULT_FREE_MODEL
                .iter()
                .find(|(p, _)| *p == provider)
                .map(|(_, m)| m.to_string())
            {
                Some(m) => m,
                None => {
                    return Err(format!(
                        "semilla {} ({}): proveedor sin modelo gratuito conocido; \
                         indica \"model\" explícitamente (no se enruta a ciegas)",
                        i, provider
                    ))
                }
            },
        };

        let caps: Vec<String> = if seed.capabilities.is_empty() {
            vec!["chat".to_string()]
        } else {
            seed.capabilities
                .iter()
                .map(|c| c.trim().to_lowercase())
                .filter(|c| !c.is_empty())
                .collect()
        };
        if caps.is_empty() {
            return Err(format!("semilla {} ({}): capacidades vacías", i, provider));
        }

        for n in 1..=seed.count {
            let id = format!("{}-{}", provider, n);
            if !seen.insert(id.clone()) {
                return Err(format!("semilla {}: id duplicado ({})", i, id));
            }
            out.push(ProviderAccount {
                id,
                provider: provider.clone(),
                tier: tier.clone(),
                model: model.clone(),
                capabilities: caps.clone(),
                // Cuota desconocida: `None` (no se inventa).  El motor trata
                // "desconocida" como mediana, no como infinita.
                remaining_quota: None,
                daily_budget_usd: 0.0,
                priority: seed.priority,
                enabled: true,
                notes: seed.notes.clone(),
            });
        }
    }

    if out.is_empty() {
        return Err("el inventario no describe ninguna cuenta".into());
    }
    Ok(out)
}

/// Carga el inventario de cuentas desde JSON.
///
/// Acepta un array de semillas o un objeto `{"accounts": [...]}` (alias
/// `providers`).  **Rechaza** cualquier JSON que traiga un campo con pinta
/// de credencial: el inventario son cuentas, no tokens.
pub fn load_inventory(json_str: &str) -> Result<Vec<ProviderAccount>, String> {
    let root: Value = serde_json::from_str(json_str)
        .map_err(|e| format!("JSON inválido: {}", e))?;

    if let Some(field) = find_secret_field(&root, "") {
        return Err(format!(
            "el inventario no puede llevar credenciales (campo {:?}); \
             los tokens se guardan aparte, fuera del repo",
            field
        ));
    }

    let arr: Vec<Value> = match &root {
        Value::Array(a) => a.clone(),
        Value::Object(o) => {
            let raw = o
                .get("accounts")
                .or_else(|| o.get("providers"))
                .ok_or("falta el array \"accounts\" (o \"providers\")")?;
            match raw {
                Value::Array(a) => a.clone(),
                _ => return Err("\"accounts\" debe ser un array".into()),
            }
        }
        _ => return Err("la raíz debe ser un array o un objeto con \"accounts\"".into()),
    };
    if arr.is_empty() {
        return Err("el inventario no describe ninguna cuenta".into());
    }

    let mut seeds: Vec<AccountSeed> = Vec::new();
    for (i, v) in arr.iter().enumerate() {
        let obj = v
            .as_object()
            .ok_or_else(|| format!("semilla {}: no es un objeto JSON", i))?;
        let provider = req_str(obj, "provider", i)?;
        let count = match obj.get("count").or_else(|| obj.get("accounts")) {
            Some(Value::Number(n)) => n
                .as_u64()
                .ok_or_else(|| format!("semilla {}: \"count\" no es un entero", i))?
                as u32,
            Some(_) => return Err(format!("semilla {}: \"count\" debe ser un entero", i)),
            None => return Err(format!("semilla {}: falta \"count\"", i)),
        };
        let tier = match obj.get("tier") {
            Some(Value::String(s)) => Some(s.trim().to_lowercase()),
            Some(_) => return Err(format!("semilla {}: \"tier\" debe ser texto", i)),
            None => None,
        };
        let model = match obj.get("model") {
            Some(Value::String(s)) => Some(s.trim().to_string()),
            Some(_) => return Err(format!("semilla {}: \"model\" debe ser texto", i)),
            None => None,
        };
        let mut capabilities = Vec::new();
        match obj.get("capabilities") {
            Some(Value::Array(a)) => {
                for c in a {
                    let s = c
                        .as_str()
                        .ok_or_else(|| format!("semilla {}: capacidades no textuales", i))?;
                    capabilities.push(s.trim().to_lowercase());
                }
            }
            Some(_) => return Err(format!("semilla {}: \"capabilities\" debe ser un array", i)),
            None => {}
        }
        let priority = match obj.get("priority") {
            Some(Value::Number(n)) => n
                .as_i64()
                .ok_or_else(|| format!("semilla {}: \"priority\" no es un entero", i))?
                as i32,
            Some(_) => return Err(format!("semilla {}: \"priority\" debe ser un entero", i)),
            None => 0,
        };
        let notes = match obj.get("notes") {
            Some(Value::String(s)) => s.trim().to_string(),
            Some(_) => return Err(format!("semilla {}: \"notes\" debe ser texto", i)),
            None => String::new(),
        };
        seeds.push(AccountSeed {
            provider,
            count,
            tier,
            model,
            capabilities,
            priority,
            notes,
        });
    }

    accounts_from_seeds(&seeds)
}

/// Inventario de cuentas en JSON (sin secretos) para poder persistirlo.
pub fn save_inventory(accounts: &[ProviderAccount]) -> String {
    let mut by_provider: Vec<(String, Vec<&ProviderAccount>)> = Vec::new();
    for a in accounts {
        match by_provider.iter_mut().find(|(p, _)| *p == a.provider) {
            Some((_, list)) => list.push(a),
            None => by_provider.push((a.provider.clone(), vec![a])),
        }
    }
    let seeds: Vec<Value> = by_provider
        .into_iter()
        .map(|(provider, list)| {
            let first = list[0];
            json!({
                "provider": provider,
                "count": list.len(),
                "tier": first.tier,
                "model": first.model,
                "capabilities": first.capabilities,
                "priority": first.priority,
                "notes": first.notes,
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({ "accounts": seeds })).unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(
        id: &str,
        tier: &str,
        model: &str,
        caps: &[&str],
        quota: Option<u32>,
        priority: i32,
    ) -> ProviderAccount {
        ProviderAccount {
            id: id.into(),
            provider: id.split('-').next().unwrap_or("x").into(),
            tier: tier.into(),
            model: model.into(),
            capabilities: caps.iter().map(|c| c.to_string()).collect(),
            remaining_quota: quota,
            daily_budget_usd: 0.0,
            priority,
            enabled: true,
            notes: String::new(),
        }
    }

    fn task(kind: &str, caps: &[&str]) -> DelegationTask {
        DelegationTask {
            id: String::new(),
            prompt: "resume el informe quarterly".into(),
            kind: kind.into(),
            required_capabilities: caps.iter().map(|c| c.to_string()).collect(),
            max_tokens: 1024,
            deadline: None,
            status: "queued".into(),
            attempts: vec![],
            assigned_account: None,
        }
    }

    fn attempt(account: &str, model: &str, ok: bool, q: Option<f32>) -> Attempt {
        Attempt {
            account: account.into(),
            model: model.into(),
            started_at: "2026-11-19T10:00:00Z".into(),
            finished_at: Some("2026-11-19T10:00:05Z".into()),
            ok,
            input_tokens: 1000,
            output_tokens: 500,
            error: if ok { None } else { Some("boom".into()) },
            quality_score: q,
        }
    }

    #[test]
    fn enrutado_prefiere_siempre_la_cuenta_gratuita_adecuada() {
        let e = DelegateEngine::new(vec![
            acc("paid-claude", "paid", "claude-sonnet-5.5", &["research"], Some(100), 90),
            acc("free-kimi", "free", "kimi-k3.1-pro", &["research"], Some(50), 0),
            acc("free-minimax", "free", "MiniMax-M3.1-Flash", &["research"], Some(50), 0),
        ]);
        let t = task("research", &["research"]);
        let picked = e.pick_account(&t).expect("debe elegir cuenta");
        // La gratuita gana siempre; entre las dos gratuitas, la de mayor cuota
        // y, a igualdad, el id alfabético decide (determinismo).
        assert_eq!(picked, "free-kimi");
        // Aunque la de pago tenga prioridad 90 y la gratuita 0.
        let e2 = DelegateEngine::new(vec![
            acc("aaa-paid", "paid", "claude-sonnet-5.5", &["code"], Some(100), 100),
            acc("zzz-free", "free", "kimi-k3.1-pro", &["code"], Some(100), 0),
        ]);
        assert_eq!(e2.pick_account(&task("code", &["code"])).unwrap(), "zzz-free");
    }

    #[test]
    fn una_cuenta_con_cuota_agotada_no_recibe_tareas() {
        let e = DelegateEngine::new(vec![
            acc("free-kimi", "free", "kimi-k3.1-pro", &["research"], Some(0), 10),
            acc("free-deepseek", "free", "deepseek-flash", &["research"], Some(30), 0),
        ]);
        let t = task("research", &["research"]);
        let picked = e.pick_account(&t).unwrap();
        assert_eq!(picked, "free-deepseek");
        // Si todas están quemadas, error explícito.
        let e2 = DelegateEngine::new(vec![acc("solo", "free", "kimi-k3", &["research"], Some(0), 0)]);
        let err = e2.pick_account(&t).unwrap_err();
        assert!(err.contains("cuota"), "el error debe mencionar la cuota: {}", err);
    }

    #[test]
    fn el_score_mejora_tras_buenos_intentos() {
        let mut e = DelegateEngine::new(vec![acc("free-kimi", "free", "kimi-k3.1-pro", &[], Some(10), 0)]);
        let before = e.score("free-kimi");
        assert!((before - QUALITY_PRIOR).abs() < 1e-6);
        let id = e.submit(task("research", &[]));
        e.record_attempt(&id, attempt("free-kimi", "kimi-k3.1-pro", true, None));
        let after = e.score("free-kimi");
        assert!(after > before, "el score debe subir: {} -> {}", before, after);
        // Un intento con calidad revisada también sube.
        e.record_attempt(&id, attempt("free-kimi", "kimi-k3.1-pro", true, Some(0.95)));
        assert!(e.score("free-kimi") > after);
        // Un fallo baja el score.
        let good = e.score("free-kimi");
        e.record_attempt(&id, attempt("free-kimi", "kimi-k3.1-pro", false, None));
        assert!(e.score("free-kimi") < good);
    }

    #[test]
    fn round_trip_de_load_y_save() {
        let original = vec![
            acc("free-kimi", "free", "kimi-k3.1-pro", &["research", "summary"], Some(42), 5),
            acc("paid-claude", "paid", "claude-sonnet-5.5", &["code"], Some(10), 20),
        ];
        let json_txt = save_accounts(&original);
        let back = load_accounts(&json_txt).expect("el JSON exportado debe reimportarse");
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].id, "free-kimi");
        assert_eq!(back[0].remaining_quota, Some(42));
        assert_eq!(back[0].capabilities, vec!["research", "summary"]);
        assert_eq!(back[1].tier, "paid");
        assert_eq!(back[1].priority, 20);
        assert_eq!(back[1].enabled, true);
    }

    #[test]
    fn las_estadisticas_cuadran() {
        let mut e = DelegateEngine::new(vec![
            acc("free-kimi", "free", "kimi-k3.1-pro", &[], Some(10), 0),
            acc("paid-claude", "paid", "claude-sonnet-5.5", &[], Some(10), 0),
        ]);
        let id = e.submit(task("research", &[]));
        e.record_attempt(&id, attempt("free-kimi", "kimi-k3.1-pro", true, None));
        e.record_attempt(&id, attempt("paid-claude", "claude-sonnet-5.5", false, None));

        let s = e.stats();
        assert_eq!(s["tokens"]["input"].as_u64().unwrap(), 2000);
        assert_eq!(s["tokens"]["output"].as_u64().unwrap(), 1000);
        assert_eq!(s["tokens"]["total"].as_u64().unwrap(), 3000);
        assert_eq!(s["attempts"]["total"].as_u64().unwrap(), 2);
        assert_eq!(s["attempts"]["ok"].as_u64().unwrap(), 1);
        assert_eq!(s["attempts"]["failed"].as_u64().unwrap(), 1);
        let rate = s["success_rate"]["overall"].as_f64().unwrap();
        assert!((rate - 0.5).abs() < 1e-9, "tasa de éxito: {}", rate);
        // La cuenta gratuita cuesta 0; la de pago cobra el precio de lista.
        let total_cost = s["cost_usd"]["estimated"].as_f64().unwrap();
        let expected = estimate_cost_usd("claude-sonnet-5.5", 1000, 500);
        assert!((total_cost - expected).abs() < 1e-12, "coste: {}", total_cost);
        // El ahorro es el precio de sombra de lo que se habría pagado.
        let shadow = s["cost_usd"]["shadow_if_paid"].as_f64().unwrap();
        let savings = s["cost_usd"]["savings"].as_f64().unwrap();
        assert!((shadow - savings - total_cost).abs() < 1e-12);
    }

    #[test]
    fn apply_adjustments_produce_el_efecto_esperado() {
        let mut e = DelegateEngine::new(vec![
            acc("free-kimi", "free", "kimi-k3.1-pro", &["research"], Some(5), 0),
            acc("paid-claude", "paid", "claude-sonnet-5.5", &["research"], Some(5), 0),
        ]);
        let id = e.submit(task("research", &["research"]));
        e.record_attempt(&id, attempt("paid-claude", "claude-sonnet-5.5", true, None));

        let adj = e.adjustments();
        assert!(!adj.is_empty(), "debe haber recomendaciones");
        // La cuenta de pago usada con alternativa gratuita debe ser relegada.
        assert!(adj
            .iter()
            .any(|a| a.account == "paid-claude"
                && matches!(a.action, AdjustmentAction::SetPriority(_))));

        e.apply_adjustments(&adj).expect("los ajustes deben aplicarse");
        assert!(e.account("paid-claude").unwrap().priority < 0);
        // La cuenta gratuita oída y sin uso debe quedar activada.
        let mut e2 = DelegateEngine::new(vec![acc("free-kimi", "free", "kimi-k3", &[], Some(5), 0)]);
        e2.accounts[0].enabled = false;
        let a2 = e2.adjustments();
        assert!(a2.iter().any(|a| matches!(a.action, AdjustmentAction::Enable)));
        e2.apply_adjustments(&a2).unwrap();
        assert!(e2.account("free-kimi").unwrap().enabled);
    }

    #[test]
    fn el_plan_ordena_y_explica() {
        let mut e = DelegateEngine::new(vec![
            acc("free-kimi", "free", "kimi-k3.1-pro", &["research", "summary"], Some(30), 0),
            acc("free-gemini", "free", "gemini-3-flash", &["research"], Some(30), 0),
        ]);
        let mut t1 = task("research", &["research"]);
        t1.id = "t-1".into();
        t1.deadline = Some("2026-11-25T00:00:00Z".into());
        let mut t2 = task("summary", &["summary"]);
        t2.id = "t-2".into();
        e.submit(t1);
        e.submit(t2);

        let plan = e.plan();
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].order, 1);
        assert_eq!(plan[0].task_id, "t-1", "la tarea con deadline va primero");
        assert!(!plan[0].account.is_empty());
        assert!(plan[0].reason.contains("gratuita"), "razón: {}", plan[0].reason);
        assert_eq!(plan[0].estimated_cost_usd, 0.0);
        assert_eq!(plan[1].task_id, "t-2");
        assert_eq!(plan[1].account, "free-kimi");
    }

    #[test]
    fn las_plantillas_existen_por_tipo_y_se_renderizan() {
        for k in ["research", "writing", "code", "summary", "audit", "classification"] {
            let t = prompt_template(k).unwrap_or_else(|| panic!("falta plantilla {}", k));
            assert!(t.contains("{prompt}"));
            let r = render_prompt(k, "ENCARGO REAL").unwrap();
            assert!(r.contains("ENCARGO REAL"));
        }
        // Alias en español.
        assert!(prompt_template("redacción").is_some());
        assert!(prompt_template("código").is_some());
        assert!(prompt_template("auditoría").is_some());
        assert!(prompt_template("clasificacion").is_some());
        // Tipo desconocido: None.
        assert!(prompt_template("cocina").is_none());
    }

    #[test]
    fn los_modelos_gratis_cuestan_cero() {
        assert_eq!(estimate_cost_usd("local-seere", 1_000_000, 1_000_000), 0.0);
        assert_eq!(estimate_cost_usd("openai/gpt-oss-120b", 1_000_000, 1_000_000), 0.0);
        assert_eq!(estimate_cost_usd("openrouter/free", 500_000, 500_000), 0.0);
        // Precio de lista real: Claude Sonnet 5.5 = 2 $/MTok in, 10 $/MTok out.
        let c = estimate_cost_usd("claude-sonnet-5.5", 1_000_000, 1_000_000);
        assert!((c - 12.0).abs() < 1e-9, "coste claude: {}", c);
        // Modelos desconocidos: estimación conservadora, no cero.
        assert!(estimate_cost_usd("modelo-inventado-9000", 1_000_000, 1_000_000) > 0.0);
        assert!(model_price("modelo-inventado-9000").is_none());
    }

    #[test]
    fn la_validacion_rechaza_presupuestos_y_cuotas_malas() {
        // Presupuesto por encima del techo.
        let bad = r#"{"accounts":[{"id":"a","provider":"kimi","tier":"free","model":"kimi-k3",
            "capabilities":["research"],"daily_budget_usd":9999.0}]}"#;
        assert!(load_accounts(bad).unwrap_err().contains("techo"));
        // Presupuesto negativo.
        let bad2 = r#"{"accounts":[{"id":"a","provider":"kimi","tier":"free","model":"kimi-k3",
            "daily_budget_usd":-1.0}]}"#;
        assert!(load_accounts(bad2).unwrap_err().contains("negativo"));
        // Cuota negativa.
        let bad3 = r#"{"accounts":[{"id":"a","provider":"kimi","tier":"free","model":"kimi-k3",
            "remaining_quota":-3}]}"#;
        assert!(load_accounts(bad3).unwrap_err().contains("negativo"));
        // Falta un campo obligatorio.
        let bad4 = r#"{"accounts":[{"provider":"kimi","tier":"free","model":"kimi-k3"}]}"#;
        assert!(load_accounts(bad4).unwrap_err().contains("id"));
        // Tier inválido.
        let bad5 = r#"{"accounts":[{"id":"a","provider":"kimi","tier":"barato","model":"kimi-k3"}]}"#;
        assert!(load_accounts(bad5).unwrap_err().contains("tier"));
        // Id duplicado.
        let bad6 = r#"{"accounts":[
            {"id":"a","provider":"kimi","tier":"free","model":"kimi-k3"},
            {"id":"a","provider":"kimi","tier":"free","model":"kimi-k3"}]}"#;
        assert!(load_accounts(bad6).unwrap_err().contains("duplicado"));
        // JSON válido.
        let ok = r#"{"accounts":[{"id":"a","provider":"kimi","tier":"free","model":"kimi-k3",
            "capabilities":["research"],"remaining_quota":10,"daily_budget_usd":1.5,
            "priority":7,"enabled":true,"notes":"web"}]}"#;
        let v = load_accounts(ok).unwrap();
        assert_eq!(v[0].priority, 7);
        assert_eq!(v[0].daily_budget_usd, 1.5);
        assert_eq!(v[0].notes, "web");
    }

    #[test]
    fn los_intentos_consumen_cuota_y_queman_las_cuentas() {
        let mut e = DelegateEngine::new(vec![acc("free-kimi", "free", "kimi-k3", &[], Some(1), 0)]);
        let id = e.submit(task("research", &[]));
        assert_eq!(e.account("free-kimi").unwrap().remaining_quota, Some(1));
        e.record_attempt(&id, attempt("free-kimi", "kimi-k3", true, None));
        assert_eq!(e.account("free-kimi").unwrap().remaining_quota, Some(0));
        // Quemada: el enrutado la excluye.
        let e2 = DelegateEngine::new(vec![acc("free-kimi", "free", "kimi-k3", &[], Some(0), 0)]);
        assert!(e2.pick_account(&task("research", &[])).is_err());
        // Y las estadísticas la marcan como quemada.
        let st = e.account_stats();
        assert!(st[0].burned);
    }

    #[test]
    fn apply_adjustments_rechaza_invalidos_sin_tocar_nada() {
        let mut e = DelegateEngine::new(vec![acc("free-kimi", "free", "kimi-k3", &[], Some(5), 3)]);
        let bad = vec![Adjustment {
            account: "no-existe".into(),
            action: AdjustmentAction::Disable,
            reason: "x".into(),
        }];
        assert!(e.apply_adjustments(&bad).is_err());
        assert!(e.account("free-kimi").unwrap().enabled, "no debe haber cambios");

        let bad2 = vec![Adjustment {
            account: "free-kimi".into(),
            action: AdjustmentAction::SetBudgetUsd(1e9),
            reason: "x".into(),
        }];
        assert!(e.apply_adjustments(&bad2).is_err());
        assert_eq!(e.account("free-kimi").unwrap().daily_budget_usd, 0.0);
    }

    #[test]
    fn el_enrutado_es_deterministico_y_puro() {
        let e = DelegateEngine::new(vec![
            acc("b-free", "free", "kimi-k3", &["research"], Some(10), 0),
            acc("a-free", "free", "kimi-k3", &["research"], Some(10), 0),
            acc("c-paid", "paid", "claude-sonnet-5.5", &["research"], Some(10), 50),
        ]);
        let t = task("research", &["research"]);
        let a = e.pick_account(&t).unwrap();
        let b = e.pick_account(&t).unwrap();
        assert_eq!(a, b);
        // Empate de score → desempate alfabético por id.
        assert_eq!(a, "a-free");
        // Y la llamada no ha mutado nada.
        assert_eq!(e.account("a-free").unwrap().remaining_quota, Some(10));
        assert_eq!(e.tasks().len(), 0);
    }

    #[test]
    fn sin_capacidades_no_hay_cuenta() {
        let e = DelegateEngine::new(vec![acc("free-kimi", "free", "kimi-k3", &["research"], Some(10), 0)]);
        let err = e.pick_account(&task("code", &["code"])).unwrap_err();
        assert!(err.contains("capacidades"), "error: {}", err);
    }

    #[test]
    fn la_cola_guarda_los_intentos_y_el_estado() {
        let mut e = DelegateEngine::new(vec![acc("free-kimi", "free", "kimi-k3", &[], Some(10), 0)]);
        let id = e.submit(task("research", &["research"]));
        assert!(!id.is_empty());
        assert_eq!(e.tasks().len(), 1);
        assert_eq!(e.task(&id).unwrap().status, "queued");
        e.record_attempt(&id, attempt("free-kimi", "kimi-k3", true, None));
        assert_eq!(e.task(&id).unwrap().status, "done");
        assert_eq!(e.task(&id).unwrap().attempts.len(), 1);
        assert_eq!(e.task(&id).unwrap().assigned_account.as_deref(), Some("free-kimi"));
        // Un fallo deja la tarea en retry hasta agotar los intentos.
        let id2 = e.submit(task("summary", &[]));
        for _ in 0..MAX_ATTEMPTS {
            e.record_attempt(&id2, attempt("free-kimi", "kimi-k3", false, None));
        }
        assert_eq!(e.task(&id2).unwrap().status, "failed");
    }

    // ── Procedencia de los precios (P1.3: "precios reales con fecha y fuente") ──

    #[test]
    fn todo_precio_de_la_tabla_tiene_fuente_y_fecha() {
        for (model, inp, out, prov) in PRICING_TABLE {
            let p = price_provenance(model)
                .unwrap_or_else(|| panic!("modelo {} sin procedencia", model));
            assert!(p.source.starts_with("https://"), "{}: fuente no URL: {}", model, p.source);
            assert!(
                parse_iso_date(&p.verified_on).is_some(),
                "{}: fecha inválida: {}",
                model,
                p.verified_on
            );
            assert_eq!(p.provider, *prov);
            assert_eq!(p.input_usd_per_mtok, *inp);
            assert_eq!(p.output_usd_per_mtok, *out);
        }
    }

    #[test]
    fn el_libro_de_precios_cubre_toda_la_tabla() {
        let ledger = pricing_ledger();
        assert_eq!(ledger.len(), PRICING_TABLE.len());
        // La procedencia de un modelo resuelve también con prefijo de vendedor.
        let p = price_provenance("openai/gpt-5.6-sol").expect("procedencia con prefijo");
        assert_eq!(p.model, "gpt-5.6-sol");
        assert!(p.note.contains("promocional"), "nota: {}", p.note);
    }

    #[test]
    fn los_precios_rancios_se_detectan_y_se_reportan() {
        let ledger = pricing_ledger();
        let fecha_max = ledger
            .iter()
            .filter_map(|p| parse_iso_date(&p.verified_on))
            .max()
            .expect("fechas");
        // Lo verificado hoy no está rancio, aunque el margen sea 0.
        let hoy = pricing_stale(0, fecha_max);
        assert!(
            hoy.iter().all(|p| p.verified_on != PRICING_VERIFIED_ON),
            "lo verificado hoy no puede estar rancio: {:?}",
            hoy
        );
        // 30 días después, lo verificado hace más de 30 días sí lo está.
        let rancios = pricing_stale(30, fecha_max + 30);
        assert!(!rancios.is_empty(), "debería detectar precios rancios");
        assert!(
            rancios.iter().all(|p| {
                days_since(&p.verified_on, fecha_max + 30).map(|d| d > 30).unwrap_or(false)
            }),
            "todos los rancios deben superar el margen: {:?}",
            rancios
        );
    }

    #[test]
    fn las_fechas_iso_se_parsean_y_se_cuentan_bien() {
        assert_eq!(parse_iso_date("1970-01-01"), Some(0));
        assert_eq!(parse_iso_date("1970-01-02"), Some(1));
        assert_eq!(parse_iso_date("2026-11-19"), Some(20_776));
        assert_eq!(parse_iso_date("2026-13-01"), None);
        assert_eq!(parse_iso_date("no-es-fecha"), None);
        assert_eq!(days_since("2026-11-19", 20_776), Some(0));
        assert_eq!(days_since("2026-11-19", 20_786), Some(10));
        // Una fecha futura no da un número negativo.
        assert_eq!(days_since("2026-11-19", 20_700), Some(0));
    }

    #[test]
    fn el_informe_de_precios_es_json_con_fuente_y_fecha() {
        let r = pricing_report(30);
        assert!(r["models"].as_u64().unwrap() as usize == PRICING_TABLE.len());
        assert!(r["ledger"].as_array().unwrap().len() == PRICING_TABLE.len());
        let primero = &r["ledger"][0];
        assert!(primero["source"].as_str().unwrap().starts_with("https://"));
        assert!(primero["verified_on"].as_str().is_some());
        assert!(r["stale_models"].as_u64().is_some());
    }

    // ── Inventario real de cuentas (P1.3 + P1.8) ──

    /// El inventario real medido en el móvil: 4 cuentas de Kimi, 3 de
    /// DeepSeek y 1 de ChatGPT (sólo recuentos, ningún secreto).
    const INVENTARIO_REAL: &str = r#"{
      "accounts": [
        {"provider": "kimi",     "count": 4, "capabilities": ["research", "chat"]},
        {"provider": "deepseek", "count": 3, "capabilities": ["code", "chat"]},
        {"provider": "chatgpt",  "count": 1, "capabilities": ["research"]}
      ]
    }"#;

    #[test]
    fn el_inventario_real_carga_las_cuentas_gratuitas() {
        let cuentas = load_inventory(INVENTARIO_REAL).expect("inventario");
        assert_eq!(cuentas.len(), 8);
        assert!(cuentas.iter().all(|c| c.tier == "free"));
        assert!(cuentas.iter().all(|c| c.remaining_quota.is_none()));
        // Modelos por defecto del proveedor.
        assert_eq!(cuentas[0].model, "kimi-k3");
        assert!(cuentas.iter().any(|c| c.model == "deepseek-chat"));
        // Ids estables y sin duplicados.
        let mut ids: Vec<&str> = cuentas.iter().map(|c| c.id.as_str()).collect();
        ids.sort();
        let antes = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), antes, "ids duplicados");
        assert!(cuentas.iter().any(|c| c.id == "kimi-1"));
        assert!(cuentas.iter().any(|c| c.id == "deepseek-3"));
    }

    #[test]
    fn el_inventario_nunca_lleva_credenciales() {
        let con_token = r#"{"accounts":[{"provider":"kimi","count":1,"token":"abc123"}]}"#;
        let err = load_inventory(con_token).unwrap_err();
        assert!(err.contains("credenciales"), "error: {}", err);

        let con_cookie = r#"[{"provider":"kimi","count":1,"cookies":{"x":"y"}}]"#;
        let err2 = load_inventory(con_cookie).unwrap_err();
        assert!(err2.contains("credenciales"), "error: {}", err2);
    }

    #[test]
    fn un_proveedor_desconocido_exige_modelo_explicito() {
        let err = load_inventory(r#"{"accounts":[{"provider":"inventado","count":1}]}"#).unwrap_err();
        assert!(err.contains("model"), "error: {}", err);

        // Con modelo explícito, se acepta.
        let ok = load_inventory(r#"{"accounts":[{"provider":"inventado","count":1,"model":"x-1"}]}"#);
        assert!(ok.is_ok(), "error: {:?}", ok.err());
    }

    #[test]
    fn el_inventario_ida_y_vuelta_conserva_las_cuentas() {
        let cuentas = load_inventory(INVENTARIO_REAL).unwrap();
        let json = save_inventory(&cuentas);
        let vuelta = load_inventory(&json).unwrap();
        assert_eq!(vuelta.len(), cuentas.len());
        assert!(vuelta.iter().all(|c| c.tier == "free"));
        assert!(vuelta.iter().all(|c| c.remaining_quota.is_none()));
    }

    #[test]
    fn el_router_enruta_al_inventario_real_por_capacidad() {
        let cuentas = load_inventory(INVENTARIO_REAL).unwrap();
        let e = DelegateEngine::new(cuentas);
        // "code" sólo lo puede hacer deepseek en este inventario.
        let elegido = e.pick_account(&task("code", &["code"])).unwrap();
        assert!(elegido.starts_with("deepseek"), "elegida: {}", elegido);
        // "research" lo pueden kimi y chatgpt; gana la de más cuota
        // (desconocida = mediana) desempatando por calidad y prioridad:
        // determinista, pero siempre dentro de las gratuitas capaces.
        let elegido2 = e.pick_account(&task("research", &["research"])).unwrap();
        assert!(
            elegido2.starts_with("kimi") || elegido2.starts_with("chatgpt"),
            "elegida: {}",
            elegido2
        );
        // Nada de pago: el inventario es gratuito.
        assert!(e.accounts().iter().all(|c| c.tier == "free"));
    }
}
