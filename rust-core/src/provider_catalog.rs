//! MegaDaemon v13.1 — Catálogo de proveedores (verificado 2026-11-19)
//!
//! POR QUÉ ESTE ARCHIVO CAMBIA ENTERO:
//!
//! El catálogo de v13.0 listaba proveedores que **ya no existen**:
//!
//!   * `minimax-m2.7` / `minimax-m2.5` → RETIRADOS el 18-nov-2026.  El
//!     modelo por defecto de MegaDaemon estaba muerto desde ayer: todas las
//!     peticiones devolvían error.  Los sustitutos son `MiniMax-M3.1-Flash`
//!     (1M de contexto, multimodal, `reasoning_effort` ajustable) y
//!     `MiniMax-M3`.
//!   * `gh_cli` / GitHub Models → servicio RETIRADO el 30-jul-2026.  El
//!     `gh auth token` sigue funcionando, pero ya no hay modelo detrás.
//!   * Pollinations, Groq, Cerebras, Google AI Studio, Mistral, OpenRouter:
//!     existían, pero el catálogo por defecto del daemon sólo contenía DOS
//!     entradas (local-seere + pollinations), así que la UI mostraba una
//!     lista vacía y el "minimax-m2.7" hardcodeado del HTML.
//!
//! VERIFICADO contra la documentación oficial (nov-2026):
//!   * GitHub Models: RETIRADO (30-jul-2026).
//!   * MiniMax M2.5/M2.7: RETIRADOS (18-nov-2026).
//!   * Cerebras: ya no hay plan gratuito permanente ($5 de crédito, con
//!     tarjeta).  Se marca `free_signup_url` pero NO se ofrece como "free".
//!   * Hugging Face Inference:Credits gratuitOS eliminados (15-oct-2026).
//!   * Groq: el plan gratuito ya NO incluye Llama; quedan gpt-oss y Qwen.
//!   * VIVOS con plan gratuito: Google AI Studio, Groq, OpenRouter `:free`,
//!     Cloudflare Workers AI, Z.ai (GLM Flash), Cohere trial, Mistral free
//!     mode, NVIDIA NIM, SambaNova, Pollinations (zero-key) y OVHcloud
//!     (anónimo, sin key).
//!
//! El daemon arranca con TODOS estos proveedores ya poblados: el usuario ve
//! la lista real en la UI y sólo tiene que pegar las keys.


use crate::providers::ProviderEntry;
/// Nivel de acceso de un proveedor.
pub fn tier_free(t: &str) -> bool {
    matches!(t, "free" | "keyless" | "offline")
}

/// Constructor de entradas del catálogo.
macro_rules! prov {
    ($id:expr, $name:expr, $vendor:expr, $endpoint:expr, $model:expr, $auth:expr, $tier:expr, $signup:expr, $reasoning:expr) => {
        ProviderEntry {
            id: $id.into(),
            name: $name.into(),
            vendor: $vendor.into(),
            endpoint: $endpoint.into(),
            model: $model.into(),
            api_key: String::new(),
            auth_type: $auth.into(),
            tier: $tier.into(),
            free_signup_url: $signup.into(),
            enabled: true,
            supports_reasoning: $reasoning,
        }
    };
}

/// Catálogo por defecto de MegaDaemon v13.1.
///
/// El orden importa: `execute_chat` construye la cascada de candidatos en el
/// orden del vault (el primero es el que se pide).  Por eso el proveedor por
/// defecto es el LOCAL (offline, gratis, sin latencia de red) y la nube
/// entra como failover.
pub fn catalog_v13_1() -> Vec<ProviderEntry> {
    vec![
        // ---------------- OFFLINE (siempre disponible) ---------------- //
        prov!(
            "local-seere",
            "SEERE v13.1 Local (Qwen/SmolLM2 + GBNF, offline)",
            "Local-Exynos",
            "http://127.0.0.1:8080/v1/chat/completions",
            "local",
            "local",
            "offline",
            "",
            false
        ),
        prov!(
            "local-seere-vllm",
            "SEERE v13.1 Local (vLLM / LM Studio / Ollama)",
            "Local-Exynos",
            "http://127.0.0.1:8000/v1/chat/completions",
            "local",
            "local",
            "offline",
            "",
            false
        ),

        // ---------------- ZERO-KEY (sin cuenta, sin key) ---------------- //
        prov!(
            "pollinations-free",
            "Pollinations AI (zero-key, sin registro)",
            "Pollinations",
            "https://text.pollinations.ai/openai",
            "openai",
            "none",
            "keyless",
            "https://pollinations.ai/",
            false
        ),
        prov!(
            "ovhcloud-free",
            "OVHcloud AI Endpoints (anónimo, 2 req/min)",
            "OVHcloud",
            "https://oai.endpoints.kepler.ai.cloud.ovh.net/v1/chat/completions",
            "gpt-4o-mini",
            "none",
            "keyless",
            "https://endpoints.ai.cloud.ovh.net/",
            false
        ),

        // ---------------- FREE CON KEY (sin tarjeta) ---------------- //
        prov!(
            "google-ai-studio",
            "Google AI Studio (Gemini 3.8 Flash, gratis sin tarjeta)",
            "Google",
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
            "gemini-3.8-flash",
            "bearer",
            "free",
            "https://aistudio.google.com/apikey",
            false
        ),
        prov!(
            "groq-free",
            "Groq (gpt-oss-120b / Qwen3.8-27B, gratis)",
            "Groq",
            "https://api.groq.com/openai/v1/chat/completions",
            "openai/gpt-oss-120b",
            "bearer",
            "free",
            "https://console.groq.com/keys",
            true
        ),
        prov!(
            "openrouter-free",
            "OpenRouter (modelos :free, 20 RPM / 50 RPD)",
            "OpenRouter",
            "https://openrouter.ai/api/v1/chat/completions",
            "openrouter/free",
            "bearer",
            "free",
            "https://openrouter.ai/keys",
            true
        ),
        prov!(
            "cloudflare-workers-ai",
            "Cloudflare Workers AI (10k neuronas/día gratis)",
            "Cloudflare",
            "https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/run/@cf/openai/gpt-oss-120b",
            "gpt-oss-120b",
            "bearer",
            "free",
            "https://dash.cloudflare.com/",
            true
        ),
        prov!(
            "zai-glm-free",
            "Z.ai (GLM-4.7-Flash / GLM-4.6V-Flash, gratis)",
            "Z.ai",
            "https://api.z.ai/api/paas/v4/chat/completions",
            "glm-4.7-flash",
            "bearer",
            "free",
            "https://z.ai/",
            true
        ),
        prov!(
            "mistral-free",
            "Mistral AI (free mode, sin tarjeta)",
            "Mistral",
            "https://api.mistral.ai/v1/chat/completions",
            "mistral-small-latest",
            "bearer",
            "free",
            "https://console.mistral.ai/",
            false
        ),
        prov!(
            "cohere-trial",
            "Cohere (trial key, 1.000 llamadas/mes)",
            "Cohere",
            "https://api.cohere.com/v2/chat",
            "command-a-03-2025",
            "bearer",
            "free",
            "https://dashboard.cohere.com/",
            false
        ),
        prov!(
            "nvidia-nim",
            "NVIDIA NIM (créditos de build, gratis)",
            "NVIDIA",
            "https://integrate.api.nvidia.com/v1/chat/completions",
            "nvidia/llama-3.3-nemotron-super-49b-v1",
            "bearer",
            "free",
            "https://build.nvidia.com/",
            true
        ),
        prov!(
            "sambanova-free",
            "SambaNova Cloud (Llama 4 Scout)",
            "SambaNova",
            "https://api.sambanova.ai/v1/chat/completions",
            "Llama-4-Scout-17B-16E-Instruct",
            "bearer",
            "free",
            "https://cloud.sambanova.ai/",
            false
        ),

        // ---------------- PAGOS / CON CRÉDITO ---------------- //
        prov!(
            "minimax-m3.1-flash",
            "MiniMax M3.1-Flash (1M ctx, multimodal, thinking ajustable)",
            "MiniMax",
            "https://api.minimax.io/v1/chat/completions",
            "MiniMax-M3.1-Flash",
            "bearer",
            "paid",
            "https://platform.minimax.io/",
            true
        ),
        prov!(
            "minimax-m3",
            "MiniMax M3 (1M ctx, multimodal)",
            "MiniMax",
            "https://api.minimax.io/v1/chat/completions",
            "MiniMax-M3",
            "bearer",
            "paid",
            "https://platform.minimax.io/",
            true
        ),
        prov!(
            "cerebras",
            "Cerebras (crédito de prueba $5, requiere tarjeta)",
            "Cerebras",
            "https://api.cerebras.ai/v1/chat/completions",
            "llama-3.3-70b",
            "bearer",
            "trial",
            "https://cloud.cerebras.ai/",
            false
        ),
        prov!(
            "deepseek",
            "DeepSeek (prepagado, sin free)",
            "DeepSeek",
            "https://api.deepseek.com/chat/completions",
            "deepseek-chat",
            "bearer",
            "paid",
            "https://platform.deepseek.com/",
            true
        ),
        prov!(
            "anthropic",
            "Anthropic Claude (API oficial)",
            "Anthropic",
            "https://api.anthropic.com/v1/messages",
            "claude-sonnet-5",
            "anthropic",
            "paid",
            "https://console.anthropic.com/",
            true
        ),
    ]
}

/// Nombres de los proveedores RETIRADOS, para avisar al usuario en vez de
/// fallar en silencio.  M2.5/M2.7 (18-nov-2026) y GitHub Models (30-jul-2026).
pub fn retired_models() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        ("minimax-m2.7", "MiniMax-M2.7", "RETIRADO el 2026-11-18 -> usa MiniMax-M3.1-Flash"),
        ("minimax-m2.7-fast", "MiniMax-M2.7-highspeed", "RETIRADO el 2026-11-18 -> usa MiniMax-M3.1-Flash"),
        ("minimax-m2.7-highspeed", "MiniMax-M2.7-highspeed", "RETIRADO el 2026-11-18 -> usa MiniMax-M3.1-Flash"),
        ("minimax-m2.5", "MiniMax-M2.5", "RETIRADO el 2026-11-18 -> usa MiniMax-M3"),
        ("minimax-m2.5-fast", "MiniMax-M2.5-highspeed", "RETIRADO el 2026-11-18 -> usa MiniMax-M3"),
        ("minimax-m2.5-highspeed", "MiniMax-M2.5-highspeed", "RETIRADO el 2026-11-18 -> usa MiniMax-M3"),
        ("github-models", "GitHub Models", "SERVICIO RETIRADO el 2026-07-30 -> no hay modelo detrás"),
        ("gh-cli", "GitHub Models (gh CLI)", "SERVICIO RETIRADO el 2026-07-30 -> usa OpenRouter o Groq"),
    ]
}

/// ¿Este id/modelo corresponde a algo ya retirado?
pub fn is_retired(id: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    retired_models().iter().find(|(i, m, _)| *i == id || *m == id)
}
