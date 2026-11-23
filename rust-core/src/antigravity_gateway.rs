//! **Antigravity como cliente de MegaDaemon**: el daemon hace de *gateway*
//! de modelos para el language server de Antigravity (la IDE agéntica de
//! Google que corre en el móvil).
//!
//! POR QUÉ EXISTE
//! -------------
//! MEDIDO (2026-11-22/23, S21 Ultra):
//!
//! * El catálogo de Antigravity es de Google/Anthropic/OpenAI y su binario
//!   **no contiene** la cadena `minimax` ⇒ para usar otros modelos hay que
//!   meter un gateway por medio.
//! * El binario del LS trae la palanca exacta:
//!   `[ModelConfig] Resolving models from Tier 3 (Gateway discovery at %s,
//!   wireProtocol: %s)` y las variables
//!   `AGY_LLM_GATEWAY_{URL,API_KEY,HEADERS,MODELS,PROXY_URL,CA_CERT,WIRE_PROTOCOL}`.
//! * El dialecto que habla Antigravity (y que ya habla nuestro gateway) es
//!   el **Gemini**: `/v1internal:generateContent`, `/v1internal:streamGenerateContent`
//!   y `/v1beta/models/{model}:generateContent`
//!   (ver `llm_gateway::gemini_route`).
//!
//! ⇒ El **cockpit server de MegaDaemon puede ser ese gateway**: ya habla
//! OpenAI *y* Gemini, ya tiene el vault de proveedores, el presupuesto y el
//! bus.  Sólo faltaba el **descubrimiento de modelos** (`GET /v1/models`),
//! que es lo que la app lee para pintar el selector, y la **instrumentación**
//! de cada llamada (que es el objetivo: verlo todo).
//!
//! QUÉ HACE ESTE MÓDULO
//! -------------------
//! 1. [`models_openai`] / [`models_gemini`]: el catálogo que Antigravity ve.
//!    Por defecto son los proveedores **listos** del vault (p. ej. MiniMax),
//!    que es exactamente lo que queremos que aparezca en el selector.
//! 2. [`instrumentar`]: cada llamada que pasa por el gateway se anexa a un
//!    NDJSON (`state/antigravity_gateway.ndjson`) con modelo, prompt, lo que
//!    se pidió y lo que se respondió — sin secretos.
//!
//! LO QUE SIGUE SIN VERIFICAR
//! --------------------------
//! Que el LS acepte de verdad `AGY_LLM_GATEWAY_URL` y pinte el catálogo del
//! gateway: **NO VERIFICADO** (el móvil estaba sin daemon cuando se escribió
//! esto).  La arquitectura está preparada y es reversible.

use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;

/// Ruta del descubrimiento de modelos en dialecto OpenAI.
pub const MODELS_OPENAI_PATH: &str = "/v1/models";
/// Ruta del descubrimiento de modelos en dialecto Gemini.
pub const MODELS_GEMINI_PATH: &str = "/v1internal:listModels";

/// Dónde se anota el tráfico del gateway (NDJSON, un evento por línea).
pub fn traza_path() -> PathBuf {
    let dir = crate::mesh::default_state_dir();
    PathBuf::from(dir).join("antigravity_gateway.ndjson")
}

/// Proveedores que el gateway ofrece a Antigravity.
///
/// Filtra los que están **listos** (con credencial) y tienen modelo.  Si el
/// vault está vacío, devuelve una lista vacía (no inventa nada).
pub fn proveedores_gateway() -> Vec<Value> {
    let vault = crate::providers::load_vault();
    let mut out = Vec::new();
    for p in &vault.providers {
        // "Listo" = habilitado y con credencial (o sin necesitarla).
        let listo = p.enabled && (!p.api_key.is_empty() || p.auth_type == "none");
        if !listo || p.model.is_empty() {
            continue;
        }
        out.push(json!({
            "id": p.id,
            "name": p.name,
            "vendor": p.vendor,
            "endpoint": p.endpoint,
            "model": p.model,
            "tier": p.tier,
        }));
    }
    out
}

/// El catálogo en dialecto OpenAI (`GET /v1/models`).
///
/// Éste es el formato que un gateway OpenAI-compatible devuelve y, por tanto,
/// el que Antigravity lee para saber qué modelos puede usar.
pub fn models_openai() -> Value {
    let data: Vec<Value> = proveedores_gateway()
        .iter()
        .map(|p| {
            let id = p.get("model").and_then(|m| m.as_str()).unwrap_or("");
            let owned = p.get("vendor").and_then(|v| v.as_str()).unwrap_or("megadaemon");
            json!({
                "id": id,
                "object": "model",
                "owned_by": owned,
                "display_name": p.get("name").and_then(|n| n.as_str()).unwrap_or(id),
                "provider_id": p.get("id").and_then(|i| i.as_str()).unwrap_or(""),
            })
        })
        .collect();
    json!({ "object": "list", "data": data })
}

/// El catálogo en dialecto Gemini (`GET /v1internal:listModels`).
///
/// Gemini nombra los modelos `models/<id>`; el `displayName` es lo que la
/// UI enseña.
pub fn models_gemini() -> Value {
    let models: Vec<Value> = proveedores_gateway()
        .iter()
        .map(|p| {
            let id = p.get("model").and_then(|m| m.as_str()).unwrap_or("");
            json!({
                "name": format!("models/{id}"),
                "displayName": p.get("name").and_then(|n| n.as_str()).unwrap_or(id),
                "supportedGenerationMethods": ["generateContent", "streamGenerateContent"],
                "version": "001",
            })
        })
        .collect();
    json!({ "models": models })
}

/// Anota una llamada que ha pasado por el gateway.
///
/// **Nunca** se guarda una credencial: sólo metadatos, longitudes y un
/// recorte del prompt (los primeros 400 caracteres) para poder ver qué
/// pidió Antigravity sin volcar el contexto entero del usuario.
pub fn instrumentar(event: &str, modelo: &str, prompt: &str, extra: Value) {
    let linea = json!({
        "ts": crate::mesh::now_iso(),
        "event": event,
        "modelo": modelo,
        "prompt_len": prompt.chars().count(),
        "prompt_head": prompt.chars().take(400).collect::<String>(),
        "extra": extra,
    });
    let p = traza_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)
    {
        let _ = writeln!(f, "{linea}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_modelos_openai_tienen_forma_de_modelo() {
        let v = models_openai();
        assert_eq!(v.get("object").and_then(|o| o.as_str()), Some("list"));
        let arr = v.get("data").and_then(|d| d.as_array()).expect("data");
        for m in arr {
            assert_eq!(m.get("object").and_then(|o| o.as_str()), Some("model"));
            assert!(!m.get("id").and_then(|i| i.as_str()).unwrap_or("").is_empty());
        }
    }

    #[test]
    fn los_modelos_gemini_se_llaman_models_mas_id() {
        let v = models_gemini();
        let arr = v.get("models").and_then(|m| m.as_array()).expect("models");
        for m in arr {
            let n = m.get("name").and_then(|x| x.as_str()).unwrap_or("");
            assert!(n.starts_with("models/"), "nombre: {n}");
        }
    }

    #[test]
    fn la_instrumentacion_no_guarda_secretos() {
        // El prompt se recorta; no hay campos de credencial en el evento.
        let largo = "secreto ".repeat(200);
        instrumentar("test", "minimax", &largo, json!({"ok": true}));
        let p = traza_path();
        if let Ok(txt) = std::fs::read_to_string(&p) {
            let ultima = txt.lines().last().unwrap_or("");
            assert!(ultima.contains("\"prompt_len\""));
            assert!(!ultima.contains("api_key"));
        }
    }
}
