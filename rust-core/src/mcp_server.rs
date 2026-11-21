//! MegaDaemon v13.0 Zenith — Native Rust Antigravity MCP Server (JSON-RPC 2.0 over stdio)
//! Integrates MiniMax-M2.7 / M2.5, Parasitic Free Swarm, DiE/Binwalk, Rizin, CIRX v3.4 SSA,
//! MBA Deobfuscation, and Exynos 2100 NPU probes directly as first-class Antigravity MCP tools!

use crate::{binwalk_die, cirx_rtl, frida_dbi, kaitai_imhex, npu_cv, providers};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

fn write_jsonrpc(val: &Value) {
    let mut out = io::stdout().lock();
    if let Ok(s) = serde_json::to_string(val) {
        let _ = writeln!(out, "{}", s);
        let _ = out.flush();
    }
}

/// Catálogo unificado de herramientas MCP (RE + control del dispositivo).
/// Las de dispositivo viven en `crate::android`; se fusionan aquí para que
/// stdio y HTTP expongan EXACTAMENTE el mismo catálogo.
pub fn tool_definitions() -> Value {
    let mut base = tool_definitions_core();
    if let Some(arr) = base.as_array_mut() {
        if let Some(dev_arr) = crate::android::tool_definitions().as_array().cloned() {
            arr.extend(dev_arr);
        }
        if let Some(sync_arr) = crate::sync::tool_definitions().as_array().cloned() {
            arr.extend(sync_arr);
        }
        if let Some(ext_arr) = crate::mcp_ext::tool_definitions().as_array().cloned() {
            arr.extend(ext_arr);
        }
    }
    base
}

fn tool_definitions_core() -> Value {
    json!([
        {
            "name": "minimax_chat",
            "description": "Invoke MiniMax-M2.7, MiniMax-M2.7-highspeed, MiniMax-M2.5, or MiniMax-M2.5-highspeed directly using the configured MiniMax API key in MegaDaemon.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "User prompt or reverse-engineering task." },
                    "model": {
                        "type": "string",
                        "description": "Provider ID: local-seere (default, offline), minimax-m3.1-flash, pollinations-free, ovhcloud-free, groq-free, google-ai-studio (modelos retirados como M2.5/M2.7 y GitHub Models NO se ofrecen)",
                        "default": "local-seere"
                    },
                    "system": { "type": "string", "description": "Optional system instruction." },
                    "attachment_path": { "type": "string", "description": "Optional path to ANY file (.7z, .exe, .elf, .md, .py, .bin) for grounded Rust DiE/Binwalk pre-analysis." },
                    "max_tokens": { "type": "integer", "default": 1024 }
                },
                "required": ["prompt"]
            }
        },
        {
            "name": "megadaemon_voice",
            "description": "La VOZ de MegaDaemon: el daemon como agente del enjambre (identidad, plan, accion y difusion). Acciones: plan | act | speak | whoami | status. Plan decide con criterio fijo o con LLM (degrada al criterio fijo si el modelo no responde). Act ejecuta por la misma registry que /api/tool/<n> (tools de lectura/accion; las destructivas estan vetadas).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["plan", "act", "speak", "whoami", "status"],
                        "default": "status"
                    },
                    "llm": { "type": "boolean", "default": false, "description": "plan: decidir con LLM en vez de con el criterio fijo." },
                    "broadcast": { "type": "boolean", "default": true, "description": "act: difundir el resultado por los 4 canales." },
                    "peer": { "type": "string", "description": "speak: nodo destino." },
                    "text": { "type": "string", "description": "speak: mensaje." },
                    "tool": { "type": "string", "description": "act/do: tool del daemon." },
                    "args": { "type": "object", "description": "act/do: argumentos de la tool." },
                    "task_id": { "type": "string", "description": "act/claim: id de la tarea." },
                    "capability": { "type": "string" }
                },
                "required": ["action"]
            }
        },
        {
            "name": "megadaemon_catalog",
            "description": "Catalogo unificado del bus de servicios: MODELOS + AGENTES/SERVICIOS con id, kind, endpoint, auth_kind, health, latency, cost_estimate, capabilities y enabled. Lo que no se puede medir va null con el motivo (nada inventado).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "description": "model | agent | service (vacio = todos)" },
                    "task": { "type": "string", "description": "pista de tarea para elegir proveedor (offline, razonamiento, ...)." }
                }
            }
        },
        {
            "name": "swarm_chat",
            "description": "Route a prompt to any model in MegaDaemon's Parasitic Swarm (MiniMax M3.1-Flash/M3, Pollinations zero-key, OpenRouter :free, Groq gpt-oss, Google AI Studio Gemini, Cloudflare Workers AI, Z.ai GLM-Flash, local SEERE v13.1).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "prompt": { "type": "string" },
                    "provider_id": { "type": "string", "default": "local-seere" },
                    "attachment_path": { "type": "string" }
                },
                "required": ["prompt"]
            }
        },
        {
            "name": "megadaemon_die_binwalk",
            "description": "Run native Rust Detect It Easy (DiE) + Binwalk + Scalpel + Bulk Extractor + Comtype Scanner on ANY file (Windows PE, Linux/Android ELF/APK/DEX, macOS Mach-O, DOS MZ/COM, 7z/ZIP, retro ROMs).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Absolute path to file to analyze." }
                },
                "required": ["file_path"]
            }
        },
        {
            "name": "megadaemon_rizin",
            "description": "Run native ARM64 Rizin 0.9.1 multi-OS binary inspection (functions, imports, exports, strings, sections, disasm, info) with zero JVM overhead.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": { "type": "string" },
                    "mode": { "type": "string", "default": "info", "description": "info | functions | imports | exports | strings | sections | disasm" },
                    "arch": { "type": "string", "default": "auto" }
                },
                "required": ["file_path"]
            }
        },
        {
            "name": "megadaemon_cirx_analyze",
            "description": "Execute LTO-Decomp v3.4 CIRX Micro-RTL SSA pipeline, Risk Score S/R, MAS Track Router, Absolute Set Closure Lemma check, and SMT-LIB2 overlap query export.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mas": { "type": "number", "default": 0.75 },
                    "m": { "type": "integer", "default": 10 },
                    "p": { "type": "number", "default": 0.05 },
                    "a": { "type": "integer", "default": 3 },
                    "l": { "type": "integer", "default": 1 },
                    "i": { "type": "integer", "default": 0 }
                }
            }
        },
        {
            "name": "megadaemon_mba_simplify",
            "description": "Algebraically simplify obfuscated Mixed Boolean-Arithmetic (MBA) expressions and opaque predicates (D810/Goomba style).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "expression": { "type": "string", "description": "e.g. (x ^ y) + 2*(x & y)" }
                },
                "required": ["expression"]
            }
        },
        {
            "name": "megadaemon_npu_cv",
            "description": "Probe Exynos 2100 NPU (/dev/vertex10, EDEN), Mali-G78 GPU, KleidiAI CPU thermals, and VLA Computer Vision telemetry.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "signing_vault",
            "description": "Vault de firma electronica de MegaDaemon (Rust puro, sin JVM): almacena y replica material criptografico (certificados X.509, CMS/CAdES SignedData, PDF PAdES-B-B) con dos zonas. Zona PUBLIC = replicable a todas las instancias; zona PRIVATE = material personal que NO sale del dispositivo. Manifiesto NDJSON indexado con SHA-256 verificado en destino (fail-closed por entrada), nombres validados sin path traversal, put idempotente y get que re-verifica el hash.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list", "put", "get", "verify", "export_public", "import_public"],
                        "default": "list"
                    },
                    "name": { "type": "string", "description": "Nombre del artefacto dentro del vault (sin rutas ni '..')." },
                    "zone": { "type": "string", "enum": ["public", "private"], "description": "public = replicable a todas las instancias; private = local, nunca sale del dispositivo." },
                    "content_b64": { "type": "string", "description": "Contenido del artefacto en base64 (para put)." },
                    "mime": { "type": "string", "description": "Tipo MIME del artefacto (opcional)." },
                    "origin": { "type": "string", "description": "Origen del artefacto (opcional)." },
                    "bundle_b64": { "type": "string", "description": "Paquete de replicacion en base64 (para import_public)." }
                }
            }
        }
    ])
}

/// Despacha una herramienta. Las de dispositivo se resuelven en
/// `crate::android::dispatch` (capa tipada con validación y auditoría).
pub fn dispatch_tool(name: &str, args: &Value) -> Value {
    if let Some(r) = crate::mcp_ext::dispatch(name, args) {
        let text = serde_json::to_string_pretty(&r).unwrap_or_default();
        return json!({
            "content": [{ "type": "text", "text": text }],
            "structuredContent": r,
            "isError": r.get("ok").and_then(|v| v.as_bool()) == Some(false),
        });
    }
    if let Some(r) = crate::sync::dispatch(name, args) {
        let text = serde_json::to_string_pretty(&r).unwrap_or_default();
        return json!({
            "content": [{ "type": "text", "text": text }],
            "structuredContent": r,
            "isError": r.get("ok").and_then(|v| v.as_bool()) == Some(false),
        });
    }
    if crate::android::tool_definitions()
        .as_array()
        .map(|a| a.iter().any(|d| d["name"].as_str() == Some(name)))
        .unwrap_or(false)
    {
        let r = crate::android::dispatch(name, args);
        // Formato MCP: el contenido va en `content[].text`; los campos
        // estructurados se añaden aparte para clientes que los saben usar.
        let text = serde_json::to_string_pretty(&r).unwrap_or_default();
        return json!({
            "content": [{ "type": "text", "text": text }],
            "structuredContent": r,
            "isError": r.get("ok").and_then(|v| v.as_bool()) == Some(false),
        });
    }
    if name == "adb" {
        let r = crate::adb_tool::dispatch(name, args)
            .unwrap_or(json!({ "ok": false, "error": "sin resultado" }));
        let text = serde_json::to_string_pretty(&r).unwrap_or_default();
        return json!({
            "content": [{ "type": "text", "text": text }],
            "structuredContent": r,
            "isError": r.get("ok").and_then(|v| v.as_bool()) == Some(false),
        });
    }
    if name == "signing_vault" {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
        let r = crate::signing_vault::handle_action(
            action,
            args,
            std::path::Path::new(&crate::cockpit_server::state_dir()),
        );
        let text = serde_json::to_string_pretty(&r).unwrap_or_default();
        return json!({
            "content": [{ "type": "text", "text": text }],
            "structuredContent": r,
            "isError": r.get("ok").and_then(|v| v.as_bool()) == Some(false),
        });
    }
    if name == "megadaemon_voice" {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("status");
        let r = crate::voice_runtime::handle(action, args);
        let text = serde_json::to_string_pretty(&r).unwrap_or_default();
        return json!({
            "content": [{ "type": "text", "text": text }],
            "structuredContent": r,
            "isError": r.get("ok").and_then(|v| v.as_bool()) == Some(false),
        });
    }
    if name == "megadaemon_catalog" {
        let kind = args.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let mut r = crate::voice_runtime::catalog();
        if !kind.is_empty() {
            if let Some(items) = r.get_mut("items").and_then(|v| v.as_array_mut()) {
                items.retain(|i| i.get("kind").and_then(|v| v.as_str()).unwrap_or("") == kind);
            }
        }
        if let Some(task) = args.get("task").and_then(|v| v.as_str()) {
            r["picked_provider"] = match crate::voice_runtime::pick_provider(task) {
                Some(p) => json!(p),
                None => json!(null),
            };
        }
        let text = serde_json::to_string_pretty(&r).unwrap_or_default();
        return json!({
            "content": [{ "type": "text", "text": text }],
            "structuredContent": r,
            "isError": false,
        });
    }
    match name {
        "minimax_chat" => {
            let prompt = args["prompt"].as_str().unwrap_or("");
            let model = args["model"].as_str().unwrap_or("local-seere");
            let sys = args["system"].as_str();
            let att = args["attachment_path"].as_str();
            let max_tok = args["max_tokens"].as_u64().unwrap_or(1024) as u32;
            let res = providers::execute_chat(prompt, Some(model), att, sys, max_tok);
            let txt = if res.reasoning.is_empty() {
                format!("[{} | {} ms | {} tok]\n{}", res.model_name, res.latency_ms, res.tokens_used, res.reply)
            } else {
                format!(
                    "[{} | {} ms | {} tok]\n<reasoning>\n{}\n</reasoning>\n\n{}",
                    res.model_name, res.latency_ms, res.tokens_used, res.reasoning, res.reply
                )
            };
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "swarm_chat" => {
            let prompt = args["prompt"].as_str().unwrap_or("");
            let pid = args["provider_id"].as_str().unwrap_or("local-seere");
            let att = args["attachment_path"].as_str();
            let res = providers::execute_chat(prompt, Some(pid), att, None, 1024);
            let txt = format!("[{} | {} ms]\n{}", res.model_name, res.latency_ms, res.reply);
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "megadaemon_die_binwalk" => {
            let fp = args["file_path"].as_str().unwrap_or("");
            let rep = binwalk_die::analyze_binary_full(fp);
            let txt = serde_json::to_string_pretty(&rep).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "megadaemon_rizin" => {
            let fp = args["file_path"].as_str().unwrap_or("");
            let mode = args["mode"].as_str().unwrap_or("info");
            let arch = args["arch"].as_str();
            let rep = frida_dbi::run_rizin_analysis(fp, mode, arch);
            let txt = serde_json::to_string_pretty(&rep).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "megadaemon_cirx_analyze" => {
            let mas = args["mas"].as_f64().unwrap_or(0.75);
            let m = args["m"].as_u64().unwrap_or(10);
            let p = args["p"].as_f64().unwrap_or(0.05);
            let a = args["a"].as_u64().unwrap_or(3);
            let l = args["l"].as_u64().unwrap_or(1);
            let i = args["i"].as_u64().unwrap_or(0);
            let rep = cirx_rtl::run_reference_pipeline(mas, m, p, a, l, i);
            let txt = serde_json::to_string_pretty(&rep).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "megadaemon_mba_simplify" => {
            let expr = args["expression"].as_str().unwrap_or("(x ^ y) + 2*(x & y)");
            let rep = kaitai_imhex::simplify_mba_expression(expr);
            let txt = serde_json::to_string_pretty(&rep).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "megadaemon_npu_cv" => {
            let rep = npu_cv::analyze_vla_frame_and_accel();
            let txt = serde_json::to_string_pretty(&rep).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        _ => {
            // v14: las tools de los módulos nuevos (apk_inventory, dex_decompile,
            // app_intel, root_hide) SÍ aparecen en tools/list (se merges con
            // `mcp_ext::tool_definitions()`), pero no tenían ruta de
            // invocación aquí => el MCP respondía "Unknown tool" a tools que
            // él mismo acababa de anunciar.  Ahora se enrutan.
            if let Some(r) = crate::mcp_ext::dispatch(name, args) {
                let text = serde_json::to_string_pretty(&r).unwrap_or_default();
                return json!({
                    "content": [{ "type": "text", "text": text }],
                    "structuredContent": r,
                    "isError": r.get("ok").and_then(|v| v.as_bool()) == Some(false),
                });
            }
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": format!("Unknown tool: {}", name) }]
            })
        }
    }
}

/// Núcleo JSON-RPC 2.0 compartido por el transporte stdio y el HTTP.
/// Devuelve `None` para notificaciones (no requieren respuesta).
pub fn handle_rpc(req: &Value) -> Option<Value> {
    let id = req.get("id").cloned();
    let method = req["method"].as_str().unwrap_or("");

    let result = match method {
        "initialize" => Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "megadaemon-zenith", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "MegaDaemon v13.3 — servidor MCP nativo: análisis RE (DiE/Binwalk/Rizin/CIRX/NPU) + control tipado del dispositivo Android (root_exec, apps, pantalla, Magisk, SELinux, props, duplicados SHA-256)."
        })),
        "ping" => Some(json!({})),
        "tools/list" => Some(json!({ "tools": tool_definitions() })),
        "tools/call" => {
            let params = &req["params"];
            let name = params["name"].as_str().unwrap_or("");
            let args = &params["arguments"];
            Some(dispatch_tool(name, args))
        }
        "resources/list" => Some(json!({ "resources": [] })),
        "prompts/list" => Some(json!({ "prompts": [] })),
        _ => None,
    };

    let id_val = match id {
        Some(v) => v,
        None => return None, // notificación
    };
    match method {
        "notifications/initialized" | "notifications/cancelled" => None,
        _ => match result {
            Some(r) => Some(json!({ "jsonrpc": "2.0", "id": id_val, "result": r })),
            None => Some(json!({
                "jsonrpc": "2.0",
                "id": id_val,
                "error": { "code": -32601, "message": format!("Method not found: {}", method) }
            })),
        },
    }
}

pub fn run_stdio_mcp_server() {
    let stdin = io::stdin();
    for line in stdin.lock().lines().flatten() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(resp) = handle_rpc(&req) {
            write_jsonrpc(&resp);
        }
    }
}
