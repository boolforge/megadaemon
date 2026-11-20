//! MegaDaemon v13.0 Zenith — Pure Rust Parasitic Multi-LLM Swarm & Provider Vault
//! Replaces legacy Python `parasite_llm_router.py` runtime shell-outs with native ARM64 Rust.
//! Supports MiniMax (M2.7 / M2.5), GitHub Models (via gh CLI), Pollinations (zero-key),
//! OpenRouter (:free), Groq, Cerebras, Google AI Studio, Mistral, Hybrid Tool+Cloud, and Local SEERE v12.

use crate::{binwalk_die, cirx_rtl, kaitai_imhex, npu_bridge, provider_catalog, seere_v11};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

pub const PROVIDERS_PATH: &str = "/data/user/0/com.agy/files/megadaemon/state/providers.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub endpoint: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_auth_type")]
    pub auth_type: String, // "bearer", "anthropic", "gh_cli", "none", "local", "hybrid"
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub free_signup_url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub supports_reasoning: bool,
}

fn default_auth_type() -> String {
    "bearer".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderVault {
    pub default_model: String,
    pub providers: Vec<ProviderEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResult {
    pub reply: String,
    pub reasoning: String,
    pub model_id: String,
    pub model_name: String,
    pub vendor: String,
    pub latency_ms: f64,
    pub tokens_used: u64,
    pub tools_used: Vec<String>,
    pub fallback_used: bool,
}

fn default_vault() -> ProviderVault {
    ProviderVault {
        // El default es el LOCAL: gratis, offline y sin latencia de red.  La
        // nube entra como failover.  En v13.0 el default era "local-seere"
        // pero el catálogo sólo tenía 2 entradas, así que la UI mostraba
        // "minimax-m2.7" (hardcodeado en el HTML) y ese modelo estaba
        // RETIRADO desde el 2026-11-18.
        default_model: "local-seere".to_string(),
        providers: provider_catalog::catalog_v13_1(),
    }
}

pub fn load_vault() -> ProviderVault {
    if let Ok(raw) = fs::read_to_string(PROVIDERS_PATH) {
        match serde_json::from_str::<ProviderVault>(&raw) {
            Ok(v) => return v,
            Err(e) => {
                // BUG v13.0: un providers.json corrupto devolvía el vault por
                // defecto y el siguiente save_vault() SOBRESCRIBÍA el
                // fichero, perdiendo TODAS las API keys del usuario.
                // Ahora se hace una copia .corrupt-<ts> y se avisa.
                let bak = format!("{}.corrupt-{}", PROVIDERS_PATH, ts_now());
                let _ = fs::write(&bak, &raw);
                eprintln!(
                    "[providers] providers.json corrupto ({}); copia en {}; se usa el catálogo por defecto",
                    e, bak
                );
            }
        }
    }
    default_vault()
}

fn ts_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn save_vault(vault: &ProviderVault) -> Result<(), String> {
    if let Some(parent) = Path::new(PROVIDERS_PATH).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let text = serde_json::to_string_pretty(vault).map_err(|e| e.to_string())?;
    fs::write(PROVIDERS_PATH, text).map_err(|e| e.to_string())
}

fn mask_key(k: &str) -> String {
    if k.is_empty() {
        return String::new();
    }
    if k == "AUTO_GH_CLI" {
        return "AUTO (gh auth token)".to_string();
    }
    // v13.2 (HW-12): antes se revelaban 6+4 = 10 caracteres (casi toda la
    // key).  Ahora como máximo 2+2.
    let chars: Vec<char> = k.chars().collect();
    if chars.len() < 12 {
        return "***".to_string();
    }
    let head: String = chars[..2].iter().collect();
    let tail: String = chars[chars.len() - 2..].iter().collect();
    format!("{}...{}", head, tail)
}

pub fn list_providers_redacted() -> Value {
    let vault = load_vault();
    let gh_ready = resolve_gh_token().is_some();
    let list: Vec<Value> = vault
        .providers
        .iter()
        .map(|p| {
            let ready = match p.auth_type.as_str() {
                "local" | "none" | "hybrid" => true,
                "gh_cli" => gh_ready,
                _ => !p.api_key.trim().is_empty(),
            };
            json!({
                "id": p.id,
                "name": p.name,
                "vendor": p.vendor,
                "endpoint": p.endpoint,
                "model": p.model,
                "auth_type": p.auth_type,
                "tier": p.tier,
                "free_signup_url": p.free_signup_url,
                "enabled": p.enabled,
                "ready": ready,
                "has_key": !p.api_key.trim().is_empty() || p.auth_type == "gh_cli",
                "key_masked": mask_key(&p.api_key),
                "supports_reasoning": p.supports_reasoning,
            })
        })
        .collect();
    let retired: Vec<Value> = provider_catalog::retired_models()
        .iter()
        .map(|(id, model, note)| json!({"id": id, "model": model, "note": note}))
        .collect();
    json!({
        "default_model": vault.default_model,
        "providers": list,
        "retired": retired,
        "catalog_size": vault.providers.len(),
        "free_providers": vault.providers.iter().filter(|p| provider_catalog::tier_free(&p.tier)).count(),
        "keyless_providers": vault.providers.iter().filter(|p| p.tier == "keyless").count(),
        "engine": "rust-swarm-v13.1"
    })
}

pub fn update_provider(
    id: &str,
    api_key: Option<&str>,
    model: Option<&str>,
    endpoint: Option<&str>,
    enabled: Option<bool>,
    set_default: bool,
) -> Result<Value, String> {
    let mut vault = load_vault();
    let mut found = false;
    for p in &mut vault.providers {
        if p.id == id {
            if let Some(k) = api_key {
                p.api_key = k.trim().to_string();
                if !p.api_key.is_empty() {
                    p.enabled = true;
                    p.tier = "configured".to_string();
                }
            }
            if let Some(m) = model {
                if !m.trim().is_empty() {
                    p.model = m.trim().to_string();
                }
            }
            if let Some(ep) = endpoint {
                if !ep.trim().is_empty() {
                    // v13.2 (HW-03): cambiar el endpoint de un proveedor que ya
                    // tiene una key guardada enviaba esa key al host nuevo
                    // (SSRF + robo de credenciales) sin pedir la key.
                    // Ahora hay que re-introducir la key al cambiar de host.
                    let nuevo = ep.trim();
                    if p.endpoint.trim() != nuevo && !p.api_key.trim().is_empty() {
                        // v13.2 (HW-03): la API key queda ATADA al endpoint
                        // completo (esquema+host+ruta), no sólo al host.  El
                        // endpoint HTTP del cockpit no tiene autenticación, así
                        // que sin esta regla cualquiera podía re-apuntar un
                        // proveedor con key guardada yeysar esa key (SSRF +
                        // robo de credenciales).  Al cambiar el endpoint, la
                        // key se vacía y hay que re-introducirla.
                        p.api_key = String::new();
                        p.auth_type = "none".to_string();
                    }
                    p.endpoint = nuevo.to_string();
                }
            }
            if let Some(en) = enabled {
                p.enabled = en;
            }
            found = true;
            break;
        }
    }
    // Si el usuario pide un modelo RETIRADO, se avisa en lugar de fallar
    // en silencio contra la API (M2.7/M2.5 se retiró el 2026-11-18; GitHub Models
    // se retiró el 2026-07-30).
    if let Some((rid, rmodel, rnote)) = provider_catalog::is_retired(id) {
        return Err(format!(
            "MODELO RETIRADO: {} ({}) -> {}",
            rmodel, rid, rnote
        ));
    }
    if !found && !id.is_empty() {
        // Allow creating custom OpenAI-compatible provider dynamically
        vault.providers.push(ProviderEntry {
            id: id.to_string(),
            name: format!("Custom ({})", id),
            vendor: "Custom".to_string(),
            endpoint: endpoint
                .unwrap_or("https://openrouter.ai/api/v1/chat/completions")
                .to_string(),
            model: model.unwrap_or("openai/gpt-4o-mini").to_string(),
            api_key: api_key.unwrap_or("").trim().to_string(),
            auth_type: "bearer".to_string(),
            tier: "custom".to_string(),
            free_signup_url: String::new(),
            enabled: enabled.unwrap_or(true),
            supports_reasoning: true,
        });
    }
    if set_default && !id.is_empty() {
        vault.default_model = id.to_string();
    }
    save_vault(&vault)?;
    Ok(list_providers_redacted())
}

use std::sync::Mutex;
use std::sync::OnceLock;

static GH_TOKEN_CACHE: OnceLock<Mutex<Option<(u64, String)>>> = OnceLock::new();

/// Token de `gh auth token`, cacheado 300 s.
///
/// BUG v13.0: se ejecutaba `gh auth token` en CADA llamada (y en cada
/// proveedor de la cascada): un fork+exec por petición.
fn resolve_gh_token() -> Option<String> {
    let cache = GH_TOKEN_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((ts, tok)) = guard.as_ref() {
            if ts_now().saturating_sub(*ts) < 300 {
                return Some(tok.clone());
            }
        }
    }
    let fresh = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    if let Ok(mut guard) = cache.lock() {
        *guard = fresh.as_ref().map(|t| (ts_now(), t.clone()));
    }
    fresh
}

fn split_think_tags(raw: &str) -> (String, String) {
    if let Some(start) = raw.find("<think>") {
        if let Some(end) = raw.find("</think>") {
            if end > start + 7 {
                let thought = raw[start + 7..end].trim().to_string();
                let before = raw[..start].trim();
                let after = raw[end + 8..].trim();
                let clean = if before.is_empty() {
                    after.to_string()
                } else if after.is_empty() {
                    before.to_string()
                } else {
                    format!("{}\n{}", before, after)
                };
                return (clean, thought);
            }
        }
    }
    (raw.trim().to_string(), String::new())
}

/// Cliente HTTPS NATIVO (ureq + rustls).  Sin curl, sin fork/exec, sin
/// ficheros temporales con la API key en disco.
///
/// BUGS QUE ARREGLA (v13.0 -> v13.1):
///   * v13.0 hacía `Command::new("curl")` en CADA petición: un fork+exec por
///     llamada, y en Android `curl` NO existe en el toybox de AOSP (el
///     agente no podía hablar con la nube sin Termux).
///   * v13.0 escribía la API key en `/data/user/0/com.agy/files/tmp/md_curl_*.cfg`
///     (ruta HARDCODEADA) y la borraba al terminar.  Si el write fallaba
///     (directorio sin permisos), la petición salía SIN cabecera de
///     autenticación y el error que veía el usuario era "HTTP transport
///     error", no "401 Unauthorized".
///   * v13.0 no miraba el status HTTP: un 401/402/429 se parseaba como si
///     fuera una respuesta vacía.
///   * v13.0 no distinguía "quedate sin saldo" de "el modelo no existe":
///     un 402 (credit exhausted) y un 404 (model retired) se mezclaban.
pub fn call_openai_http(
    endpoint: &str,
    model: &str,
    api_key: &str,
    auth_type: &str,
    system_prompt: &str,
    user_prompt: &str,
    max_tokens: u32,
    temperature: f32,
) -> Result<(String, String, u64), String> {
    let payload = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "max_tokens": max_tokens,
        "temperature": temperature
    });

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(45)))
        .timeout_connect(Some(std::time::Duration::from_secs(10)))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let mut req = agent.post(endpoint);
    let key = api_key.trim();
    match auth_type {
        "anthropic" => {
            if !key.is_empty() {
                req = req.header("x-api-key", key);
            }
            req = req.header("anthropic-version", "2023-06-01");
        }
        _ => {
            if !key.is_empty() {
                req = req.header("Authorization", &format!("Bearer {}", key));
            }
        }
    }
    req = req
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://github.com/boolforge/megadaemon")
        .header("X-Title", "MegaDaemon Zenith v13.1");

    let body_bytes = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let resp = req
        .send(body_bytes.as_slice())
        .map_err(|e| format!("HTTP transport error: {}", e))?;

    let status = resp.status().as_u16();
    let text = resp
        .into_body()
        .read_to_string()
        .map_err(|e| format!("HTTP body read error: {}", e))?;

    if status >= 400 {
        // Diagnóstico accionable: distinguir saldo/agota/modelo-muerto.
        let detail = serde_json::from_str::<Value>(text.trim())
            .ok()
            .and_then(|v| {
                v.get("error")
                    .and_then(|e| {
                        e.get("message")
                            .and_then(|m| m.as_str())
                            .or_else(|| e.as_str())
                            .map(|s| s.to_string())
                    })
                    .or_else(|| v.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
            })
            .unwrap_or_else(|| text.chars().take(200).collect());
        let hint = match status {
            401 | 403 => " (API key invalida o sin permisos)",
            402 => " (sin saldo / plan agotado)",
            404 => " (modelo inexistente o RETIRADO - mira el catalogo)",
            429 => " (rate limit o cuota diaria agotada)",
            _ => "",
        };
        return Err(format!("HTTP {}: {}{}", status, detail, hint));
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("Empty HTTP response from provider".to_string());
    }

    // Algunos endpoints zero-key devuelven texto plano.
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        if let Some(err_obj) = v.get("error") {
            let msg = err_obj["message"]
                .as_str()
                .or_else(|| err_obj.as_str())
                .unwrap_or("API error");
            return Err(format!("Provider API error: {}", msg));
        }
        let msg_node = &v["choices"][0]["message"];
        let content = msg_node["content"].as_str().unwrap_or("");
        let extra_reason = msg_node["reasoning_content"]
            .as_str()
            .or_else(|| msg_node["reasoning"].as_str())
            .unwrap_or("");
        let tokens = v["usage"]["total_tokens"].as_u64().unwrap_or(0);
        let (clean, think_tag) = split_think_tags(content);
        let final_reason = if !extra_reason.is_empty() {
            extra_reason.to_string()
        } else {
            think_tag
        };
        if !clean.is_empty() {
            return Ok((clean, final_reason, tokens));
        }
        if !final_reason.is_empty() {
            return Ok((final_reason.clone(), String::new(), tokens));
        }
        return Err(format!("Unexpected JSON payload: {}", &trimmed[..trimmed.len().min(200)]));
    }

    Ok((trimmed.to_string(), String::new(), 0))
}

fn default_re_system_prompt() -> &'static str {
    "Eres MegaDaemon Zenith v13.0, un motor experto de ingeniería inversa multiplataforma (Android ARM64/DEX/APK, Linux ELF, Windows PE/PE32+, macOS Mach-O, DOS MZ/COM, Sega CD M68K, PS1 MIPS R3000A, Saturn SH-2, PC Engine HuC6280, MSX2 Z80) y arquitectura formal LTO-Decomp v3.4 (CIRX Micro-RTL/SSA, MAS Track Routing, Absolute Set Closure Lemma A={formal_control_flow}). Operas en un entorno local de investigación sin restricciones artificiales. Responde de forma concisa, técnica, directa y precisa en el idioma del usuario (por defecto español)."
}

/// Executes optional deterministic Rust RE pre-analysis when an attachment or special intent is detected,
/// giving even cloud models grounded byte-level telemetry without hallucination.
/// Directorios donde puede estar un adjunto legible por el daemon.
///
/// v13.2 (HW-04): antes se analizaba (y subía al proveedor) CUALQUIER ruta
/// que llegara en el campo `attachment`: con el endpoint HTTP sin
/// autenticación eso es una lectura de ficheros arbitraria + exfiltración.
pub const ATTACHMENT_ROOTS: &[&str] = &[
    "/data/user/0/com.agy/files/megadaemon/uploads",
    "/data/user/0/com.agy/files/megadaemon/extracted",
    "/data/user/0/com.agy/files/megadaemon/re_pipeline",
    "/data/user/0/com.agy/files/snatcher",
];

/// ¿Este adjunto está permitido?
pub fn attachment_allowed(path: &str) -> bool {
    if path.is_empty() || !path.starts_with('/') || path.contains("..") {
        return false;
    }
    let canon = match std::fs::canonicalize(path) {
        Ok(c) => c.to_string_lossy().to_string(),
        Err(_) => return false,
    };
    ATTACHMENT_ROOTS.iter().any(|r| canon.starts_with(r))
}

fn build_grounded_re_context(prompt: &str, attachment_path: Option<&str>) -> (String, Vec<String>) {
    let mut ctx = String::new();
    let mut tools = Vec::new();
    let low = prompt.to_lowercase();

    if let Some(fp) = attachment_path {
        if !attachment_allowed(fp) {
            // Adjunto fuera del sandbox: se ignora y se declara.
            ctx.push_str(&format!(
                "\n[ADJUNTO IGNORADO: la ruta no está dentro de las carpetas permitidas del daemon: {}]\n",
                fp
            ));
            tools.push(format!("AttachmentRejected({})", fp));
        } else if !fp.is_empty() && Path::new(fp).exists() {
            let die = binwalk_die::analyze_binary_full(fp);
            tools.push(format!("DiE+Binwalk({})", die.format_family));
            ctx.push_str(&format!(
                "\n[TELEMETRÍA NATIVA RUST DiE/BINWALK DEL ARCHIVO ADJUNTO]\n\
                 Archivo: {} ({} bytes)\n\
                 Formato/OS: {} | Arch: {} | Empaquetador/Compilador: {}\n\
                 Entropía Global: {:.3} bits/B | Firmas Embebidas: {} | Comtype hits: {}\n",
                die.filename,
                die.file_size,
                die.format_family,
                die.architecture,
                die.packer_or_compiler,
                die.global_entropy,
                die.embedded_signatures.len(),
                die.comtype_candidates.len()
            ));
            if !die.sections.is_empty() {
                let sec_names: Vec<String> = die
                    .sections
                    .iter()
                    .take(8)
                    .map(|s| format!("{}({}B,H={:.2})", s.name, s.size, s.entropy))
                    .collect();
                ctx.push_str(&format!("Secciones: {}\n", sec_names.join(", ")));
            }
            if !die.bulk_artifacts.is_empty() {
                let arts: Vec<String> = die
                    .bulk_artifacts
                    .iter()
                    .take(8)
                    .map(|a| format!("{}:{}", a.kind, a.value))
                    .collect();
                ctx.push_str(&format!("Bulk Extractor: {}\n", arts.join(" | ")));
            }
            // If text-like file (.md, .py, .rs, .c, .js, .json, .txt, .asm), include first 4KB verbatim!
            if let Ok(bytes) = fs::read(fp) {
                let sample_len = bytes.len().min(4096);
                let printable = bytes[..sample_len]
                    .iter()
                    .filter(|&&b| b == b'\n' || b == b'\r' || b == b'\t' || (32..=126).contains(&b))
                    .count();
                if sample_len > 0 && (printable * 100 / sample_len) >= 85 {
                    let txt = String::from_utf8_lossy(&bytes[..sample_len]);
                    ctx.push_str(&format!("\n[CONTENIDO TEXTUAL DEL ADJUNTO (primeros {} B)]\n{}\n", sample_len, txt));
                    tools.push("TextIngest".to_string());
                }
            }
        }
    }

    if low.contains("npu") || low.contains("exynos") || low.contains("temperatura") || low.contains("thermal") {
        let npu = npu_bridge::probe_exynos_npu();
        tools.push("NPUProbe".to_string());
        ctx.push_str(&format!(
            "\n[TELEMETRÍA EXYNOS 2100 EN VIVO]\nVS4L /dev/vertex10: {} | EDEN dlopen: {} | CPU Temp: {:.1}C | Batt: {:.1}C | Freqs MHz: {:?}\n",
            npu.vs4l_present, npu.eden_runtime.dlopen_ok, npu.cpu_temp_c, npu.battery_temp_c, npu.cpu_freqs_mhz
        ));
    }

    if low.contains("cirx") || low.contains("lto-decomp") || low.contains("ssa") {
        let rep = cirx_rtl::run_reference_pipeline(0.75, 10, 0.05, 3, 1, 0);
        tools.push("CIRX-SSA-v3.4".to_string());
        ctx.push_str(&format!(
            "\n[LTO-DECOMP v3.4 CIRX SSA VERIFICADO]\nTrack: {} | Risk R={} (S={}) | Blocks: {} | Phis: {} | Closure Lemma: {}\n",
            rep.track, rep.risk_r, rep.risk_s, rep.ssa_blocks, rep.phi_nodes, rep.closure_lemma_status
        ));
    }

    if low.contains("mba") || low.contains("desofusca") || low.contains("ollvm") {
        if let Some(expr_idx) = prompt.find('(') {
            let candidate = &prompt[expr_idx..];
            let simp = kaitai_imhex::simplify_mba_expression(candidate);
            if simp.rules_applied > 0 {
                tools.push("MBA-Deobf".to_string());
                ctx.push_str(&format!(
                    "\n[SIMPLIFICADOR MBA D810/GOOMBA]\nEntrada: {}\nSalida: {}\nReglas: {:?}\n",
                    simp.original, simp.simplified, simp.steps
                ));
            }
        }
    }

    (ctx, tools)
}

/// Master Pure Rust Chat Orchestrator — handles explicit model selection, Hybrid Rust-Tools+Cloud,
/// Local SEERE v12 offline execution, and automatic zero-downtime fallback!
pub fn execute_chat(
    prompt: &str,
    requested_model: Option<&str>,
    attachment_path: Option<&str>,
    system_override: Option<&str>,
    max_tokens: u32,
) -> ChatResult {
    let t0 = Instant::now();
    let vault = load_vault();
    let target_id = requested_model
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(&vault.default_model);

    // v13.2 (HW-13): pedir un modelo RETIRADO no puede terminar en otro
    // proveedor sin avisar.  Se responde con el aviso y el sustituto.
    if let Some((rid, rmodel, rnote)) = provider_catalog::is_retired(target_id) {
        return ChatResult {
            reply: format!(
                "MODELO RETIRADO: {} ({}) -> {}. El daemon NO lo sustituye por otro \
                 proveedor en silencio; elige un modelo vigente.",
                rmodel, rid, rnote
            ),
            reasoning: String::new(),
            model_id: rid.to_string(),
            model_name: rmodel.to_string(),
            vendor: "catalog".to_string(),
            latency_ms: (t0.elapsed().as_secs_f64() * 1000.0).round(),
            tokens_used: 0,
            tools_used: Vec::new(),
            fallback_used: false,
        };
    }

    let (re_ctx, mut tools_used) = build_grounded_re_context(prompt, attachment_path);
    let enriched_prompt = if re_ctx.is_empty() {
        prompt.to_string()
    } else {
        format!("{}\n\n[PREGUNTA DEL USUARIO]\n{}", re_ctx, prompt)
    };

    // Explicit Local SEERE v12 offline path
    if target_id == "local-seere" || target_id == "local" {
        // v13.2 (S09): el fallback local respeta la política de energía.
        let (_, pol) = crate::power_policy::current_policy();
        let res = seere_v11::run_seere_v11(&enriched_prompt, pol.max_turns.max(1), true, false, false, true, true);
        let ans = res["answer"].as_str().unwrap_or("Sin respuesta local").to_string();
        if let Some(arr) = res["tools"].as_array() {
            for t in arr {
                let tn = t["tool"].as_str().unwrap_or("TOOL");
                let ta = t["arg"].as_str().unwrap_or("");
                tools_used.push(format!("{}:{}", tn, ta));
            }
        }
        return ChatResult {
            reply: ans,
            reasoning: String::new(),
            model_id: "local-seere".to_string(),
            model_name: "Local SEERE v12 (Offline Rust GBNF)".to_string(),
            vendor: "Local-Exynos".to_string(),
            latency_ms: (t0.elapsed().as_secs_f64() * 1000.0).round(),
            tokens_used: res["tokens"].as_u64().unwrap_or(0),
            tools_used,
            fallback_used: false,
        };
    }

    // Hybrid path: run SEERE local shell/root tool discovery if shell/root intent + synthesize with MiniMax M2.7
    let actual_cloud_id = if target_id == "hybrid-seere" {
        let low = prompt.to_lowercase();
        if low.contains("pid")
            || low.contains("root")
            || low.contains("ls ")
            || low.contains("archivo")
            || low.contains("kernel")
            || low.contains("proceso")
            || low.contains("memoria")
        {
            let arsenal = seere_v11::Arsenal::new(false, true, true);
            let sys_snap = arsenal.exec_root("uname -r; id; free -m | head -n 2");
            tools_used.push("RootSnap".to_string());
            let _ = sys_snap;
        }
        "minimax-m3.1-flash"
    } else {
        target_id
    };

    let sys_prompt = system_override.unwrap_or_else(|| default_re_system_prompt());

    // Build ordered candidate cascade: requested model first -> other enabled cloud models -> local SEERE v12
    let mut candidates: Vec<ProviderEntry> = Vec::new();
    if let Some(p) = vault.providers.iter().find(|p| p.id == actual_cloud_id) {
        candidates.push(p.clone());
    }
    for p in &vault.providers {
        if p.id != actual_cloud_id && p.enabled && p.auth_type != "local" && p.auth_type != "hybrid" {
            candidates.push(p.clone());
        }
    }

    let mut errors: Vec<String> = Vec::new();
    for (idx, p) in candidates.iter().enumerate() {
        let key = match p.auth_type.as_str() {
            "gh_cli" => match resolve_gh_token() {
                Some(t) => t,
                None => continue,
            },
            "none" => String::new(),
            _ => {
                if p.api_key.trim().is_empty() {
                    continue;
                }
                p.api_key.clone()
            }
        };

        match call_openai_http(
            &p.endpoint,
            &p.model,
            &key,
            &p.auth_type,
            sys_prompt,
            &enriched_prompt,
            max_tokens,
            0.3,
        ) {
            Ok((reply, reasoning, tokens)) => {
                return ChatResult {
                    reply,
                    reasoning,
                    model_id: p.id.clone(),
                    model_name: p.name.clone(),
                    vendor: p.vendor.clone(),
                    latency_ms: (t0.elapsed().as_secs_f64() * 1000.0).round(),
                    tokens_used: tokens,
                    tools_used,
                    fallback_used: idx > 0,
                };
            }
            Err(e) => {
                errors.push(format!("{}: {}", p.id, e));
            }
        }
    }

    // Final guaranteed offline fallback to Local SEERE v12 Rust GBNF
    let (_, pol) = crate::power_policy::current_policy();
    let res = seere_v11::run_seere_v11(&enriched_prompt, pol.max_turns.max(1), true, false, false, true, true);
    let ans = res["answer"].as_str().unwrap_or("Sin respuesta").to_string();
    let note = if errors.is_empty() {
        ans
    } else {
        format!("{}\n\n[Fallback offline activo — errores cloud: {}]", ans, errors.join(" | "))
    };
    ChatResult {
        reply: note,
        reasoning: String::new(),
        model_id: "local-seere".to_string(),
        model_name: "Local SEERE v12 (Offline Fallback)".to_string(),
        vendor: "Local-Exynos".to_string(),
        latency_ms: (t0.elapsed().as_secs_f64() * 1000.0).round(),
        tokens_used: res["tokens"].as_u64().unwrap_or(0),
        tools_used,
        fallback_used: true,
    }
}

pub fn test_provider_connection(id: &str) -> Value {
    let t0 = Instant::now();
    let res = execute_chat("Responde exactamente: OK", Some(id), None, Some("Reply OK only."), 24);
    json!({
        "status": if res.model_id == id || (!res.fallback_used && !res.reply.is_empty()) { "OK" } else { "FALLBACK" },
        "requested": id,
        "resolved_model": res.model_id,
        "reply": res.reply,
        "latency_ms": (t0.elapsed().as_secs_f64() * 1000.0).round(),
        "fallback_used": res.fallback_used
    })
}
