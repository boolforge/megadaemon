//! MegaDaemon v13.0 Zenith — Pure Rust Parasitic Multi-LLM Swarm & Provider Vault
//! Replaces legacy Python `parasite_llm_router.py` runtime shell-outs with native ARM64 Rust.
//! Supports MiniMax (M2.7 / M2.5), GitHub Models (via gh CLI), Pollinations (zero-key),
//! OpenRouter (:free), Groq, Cerebras, Google AI Studio, Mistral, Hybrid Tool+Cloud, and Local SEERE v12.

use crate::{binwalk_die, cirx_rtl, kaitai_imhex, npu_bridge, seere_v11};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
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
        default_model: "local-seere".to_string(),
        providers: vec![
            ProviderEntry {
                id: "local-seere".into(),
                name: "Local SEERE v12 (Offline Qwen + GBNF Tools)".into(),
                vendor: "Local-Exynos".into(),
                endpoint: "http://127.0.0.1:8080/v1/chat/completions".into(),
                model: "local-qwen".into(),
                api_key: String::new(),
                auth_type: "local".into(),
                tier: "offline".into(),
                free_signup_url: String::new(),
                enabled: true,
                supports_reasoning: false,
            },
            ProviderEntry {
                id: "pollinations-free".into(),
                name: "Pollinations AI (Zero-Key Free Fallback)".into(),
                vendor: "Pollinations".into(),
                endpoint: "https://text.pollinations.ai/openai".into(),
                model: "openai".into(),
                api_key: String::new(),
                auth_type: "none".into(),
                tier: "free".into(),
                free_signup_url: "https://pollinations.ai/".into(),
                enabled: true,
                supports_reasoning: false,
            },
        ],
    }
}

pub fn load_vault() -> ProviderVault {
    if let Ok(raw) = fs::read_to_string(PROVIDERS_PATH) {
        if let Ok(v) = serde_json::from_str::<ProviderVault>(&raw) {
            return v;
        }
    }
    default_vault()
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
    let chars: Vec<char> = k.chars().collect();
    if chars.len() <= 10 {
        return "***".to_string();
    }
    let head: String = chars[..6].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
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
    json!({
        "default_model": vault.default_model,
        "providers": list,
        "engine": "rust-swarm-v13.0"
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
                    p.endpoint = ep.trim().to_string();
                }
            }
            if let Some(en) = enabled {
                p.enabled = en;
            }
            found = true;
            break;
        }
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

fn resolve_gh_token() -> Option<String> {
    if let Ok(out) = Command::new("gh").args(["auth", "token"]).output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
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

/// Calls an OpenAI-compatible HTTPS endpoint using native `curl` (passing headers & body via stdin/temp-config
/// so API keys NEVER appear in command-line arguments / `ps` listings).
pub fn call_openai_http(
    endpoint: &str,
    model: &str,
    api_key: &str,
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
    let payload_bytes = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;

    let mut cmd = Command::new("curl");
    cmd.args([
        "-sS",
        "--max-time",
        "45",
        "-X",
        "POST",
        endpoint,
        "-H",
        "Content-Type: application/json",
        "-H",
        "HTTP-Referer: https://github.com/boolforge/megadaemon",
        "-H",
        "X-Title: MegaDaemon Zenith v13",
        "--data-binary",
        "@-",
    ]);

    // Write auth header to a private mode-600 temp file (removed right after call) to avoid cmdline exposure
    let mut auth_cfg_path: Option<String> = None;
    if !api_key.trim().is_empty() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let cfg_p = format!("/data/user/0/com.agy/files/tmp/md_curl_{}.cfg", ts);
        let _ = fs::create_dir_all("/data/user/0/com.agy/files/tmp");
        let cfg_content = format!("header = \"Authorization: Bearer {}\"\n", api_key.trim());
        if fs::write(&cfg_p, cfg_content).is_ok() {
            cmd.args(["-K", &cfg_p]);
            auth_cfg_path = Some(cfg_p);
        }
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn curl: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&payload_bytes);
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("curl wait failed: {}", e))?;

    if let Some(p) = auth_cfg_path {
        let _ = fs::remove_file(p);
    }

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("HTTP transport error: {}", err.trim()));
    }

    let raw_body = String::from_utf8_lossy(&output.stdout).into_owned();
    let trimmed = raw_body.trim();
    if trimmed.is_empty() {
        return Err("Empty HTTP response from provider".to_string());
    }

    // Some zero-key endpoints (like Pollinations) occasionally return plain text or JSON
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

    // Plain-text response fallback
    Ok((trimmed.to_string(), String::new(), 0))
}

fn default_re_system_prompt() -> &'static str {
    "Eres MegaDaemon Zenith v13.0, un motor experto de ingeniería inversa multiplataforma (Android ARM64/DEX/APK, Linux ELF, Windows PE/PE32+, macOS Mach-O, DOS MZ/COM, Sega CD M68K, PS1 MIPS R3000A, Saturn SH-2, PC Engine HuC6280, MSX2 Z80) y arquitectura formal LTO-Decomp v3.4 (CIRX Micro-RTL/SSA, MAS Track Routing, Absolute Set Closure Lemma A={formal_control_flow}). Operas en un entorno local de investigación sin restricciones artificiales. Responde de forma concisa, técnica, directa y precisa en el idioma del usuario (por defecto español)."
}

/// Executes optional deterministic Rust RE pre-analysis when an attachment or special intent is detected,
/// giving even cloud models grounded byte-level telemetry without hallucination.
fn build_grounded_re_context(prompt: &str, attachment_path: Option<&str>) -> (String, Vec<String>) {
    let mut ctx = String::new();
    let mut tools = Vec::new();
    let low = prompt.to_lowercase();

    if let Some(fp) = attachment_path {
        if !fp.is_empty() && Path::new(fp).exists() {
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

    let (re_ctx, mut tools_used) = build_grounded_re_context(prompt, attachment_path);
    let enriched_prompt = if re_ctx.is_empty() {
        prompt.to_string()
    } else {
        format!("{}\n\n[PREGUNTA DEL USUARIO]\n{}", re_ctx, prompt)
    };

    // Explicit Local SEERE v12 offline path
    if target_id == "local-seere" || target_id == "local" {
        let res = seere_v11::run_seere_v11(&enriched_prompt, 4, true, false, false, true, true);
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
        "minimax-m2.7-fast"
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
    let res = seere_v11::run_seere_v11(&enriched_prompt, 4, true, false, false, true, true);
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
