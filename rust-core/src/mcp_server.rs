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

fn tool_definitions() -> Value {
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
        }
    ])
}

fn dispatch_tool(name: &str, args: &Value) -> Value {
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
        _ => json!({
            "isError": true,
            "content": [{ "type": "text", "text": format!("Unknown tool: {}", name) }]
        }),
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
        let id = req.get("id").cloned();
        let method = req["method"].as_str().unwrap_or("");

        match method {
            "initialize" => {
                if let Some(id_val) = id {
                    write_jsonrpc(&json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "result": {
                            "protocolVersion": "2024-11-05",
                            "capabilities": { "tools": {} },
                            "serverInfo": {
                                "name": "megadaemon-minimax-zenith",
                                "version": "13.0.0"
                            }
                        }
                    }));
                }
            }
            "notifications/initialized" | "notifications/cancelled" => {
                // Notification — no response required
            }
            "ping" => {
                if let Some(id_val) = id {
                    write_jsonrpc(&json!({ "jsonrpc": "2.0", "id": id_val, "result": {} }));
                }
            }
            "tools/list" => {
                if let Some(id_val) = id {
                    write_jsonrpc(&json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "result": { "tools": tool_definitions() }
                    }));
                }
            }
            "tools/call" => {
                if let Some(id_val) = id {
                    let params = &req["params"];
                    let name = params["name"].as_str().unwrap_or("");
                    let args = &params["arguments"];
                    let res = dispatch_tool(name, args);
                    write_jsonrpc(&json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "result": res
                    }));
                }
            }
            _ => {
                if let Some(id_val) = id {
                    write_jsonrpc(&json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "error": { "code": -32601, "message": format!("Method not found: {}", method) }
                    }));
                }
            }
        }
    }
}
