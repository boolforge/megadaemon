// El binario es un envoltorio fino: toda la lógica vive en la lib
// (megadaemon_core), que es lo que permite `cargo test`.
use megadaemon_core::*;

use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};

fn print_usage() {
    eprintln!("MegaDaemon Native Rust Core v13.1 Zenith (Pure Rust ARM64 + Exynos 2100 NPU + LTO-Decomp v3.4 CIRX)");
    eprintln!("Usage: megadaemon-core <command> [args...]");
    eprintln!("Commands:");
    eprintln!("  cockpit [port]                              Launch 100% Pure-Rust Multi-Threaded Cockpit HTTP Server (default: 3000)");
    eprintln!("  mcp-server                                  Run Antigravity JSON-RPC 2.0 MCP Server over stdio (MiniMax + RE Tools)");
    eprintln!("  chat <model_id> <prompt>                    Query any catalog provider (MiniMax M3.1-Flash/M3, Groq, OpenRouter, Google AI Studio, ...) or Local SEERE");
    eprintln!("  die-binwalk <file>                          Detect It Easy + Binwalk + Scalpel + Bulk Extractor + Comtype Scanner");
    eprintln!("  cirx-ssa [mas]                              LTO-Decomp v3.4 CIRX Micro-RTL SSA, Risk Score S/R & SMT-LIB2 Export");
    eprintln!("  mba <expr>                                  D810/Goomba Mixed Boolean-Arithmetic (MBA) Algebraic Simplifier");
    eprintln!("  kaitai <file> [template] [offset]           ImHex/Kaitai Struct Declarative Binary Parser");
    eprintln!("  rizin <file> [mode] [arch]                  Native ARM64 Rizin 0.9.1 Multi-OS Inspector");
    eprintln!("  frida <mode> <module> <sym_or_off> [proc]   Frida DBI Script Generator (attach|stalker|native_call|android_jni|tenet_mem)");
    eprintln!("  npu-cv                                      Exynos 2100 NPU + Mali-G78 + VLA Computer Vision ROI Telemetry");
    eprintln!("  unifier                                     Scan & Unify All 9 Projects Across the Device Ecosystem");
    eprintln!("  rea <plat> [query] [max]                    REA Ghidra 2.7MB C Corpus Indexer & ScummVM C++ Transpiler");
    eprintln!("  cfg <plat> [offset] [count]                 Native Control Flow Graph (CFG) Lifter & Mermaid Generator");
    eprintln!("  bindiff <plat>                              Multi-ISA Cross-Platform Function Matcher");
    eprintln!("  hw-mmio <plat>                              Hardware MMIO Register Annotator & Auto-Struct Reconstructor");
    eprintln!("  crypto-scan <plat>                          Shift-JIS String Decoder & Konami LZSS/RLE Scanner");
    eprintln!("  seere <query> [--max-turns N] [--env-in-prompt]   Execute SEERE v13.1 GBNF Local Harness (0 tokens if possible)");
    eprintln!("  seere-prompt [es|en]                         Dump the static SEERE system prompt (for token accounting)");
    eprintln!("  seere-grammar                                Dump the active GBNF grammar");
    eprintln!("  power                                        Show battery/charging/thermal state and the active policy");
    eprintln!("  seere-selftest                              Run SEERE v12 Assertion Suites & Export Shared GBNF/Prompts");
    eprintln!("  npu-probe                                   Probe Exynos 2100 NPU (/dev/vertex10, EDEN, KleidiAI) & Pin Cores 4-7");
    eprintln!("  npu-probe --vs4l                            Sondea el driver VS4L/NPU (ioctl SIN argumento => devuelve el tamano que espera)");
    eprintln!("  csc-verify                                Verifica en el movil si la feature de grabacion nativa (CSC) esta activa");
    eprintln!("  locate-roms [refresh]                       Native Multi-Platform ROM & Binary Auto-Discovery");
    eprintln!("  disasm <plat> <file> [offset] [count]       Native Multi-Arch Disassembler");
    eprintln!("  hex <file> [offset] [length]                Native Hex & ASCII Inspector");
    eprintln!("  deobfuscate <plat> <file>                   Native Konami XOR Script Deobfuscator");
    eprintln!("  dumper <plat> <file>                        Native Asset Signature Extractor");
    eprintln!("  entropy <file> [block_size]                 Shannon Entropy Block Classifier");
    eprintln!("  pattern <file> <pattern_str> [max_matches]  Wildcard AOB Pattern Scanner");
    eprintln!("  pointers <file> <base_hex> [is_be] [max]    32-bit Pointer Table Miner");
    eprintln!("  diff <file_a> <file_b> [base_hex] [max]     Memory Snapshot Differential Engine");
    eprintln!("  shm-status                                  Zero-Copy VLA SHM Bus Header Status");
    eprintln!("  shm-frame-bmp                               Render Live 24-bit BMP Frame from SHM");
}

/// D-10 (P0): hook de panic con backtrace.
///
/// El release usa `panic = "unwind"` (v15), asi que un panic en un hilo de
/// peticion lo captura `catch_unwind` (=> la peticion responde 500 y el
/// daemon sigue) pero un panic en un hilo de fondo **mata el proceso**
/// (=> el "134" que se vio en el movil, que NO puede ser un panic Rust).
/// Con este hook cada panic deja rastro real: mensaje, hilo y backtrace.
///
/// El backtrace se captura SIEMPRE (no solo con RUST_BACKTRACE=1): en el
/// movil nadie pone esa variable y sin esto el 134 es indescifrable.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let hilo = std::thread::current()
            .name()
            .unwrap_or("<sin nombre>")
            .to_string();
        let backtrace = std::backtrace::Backtrace::force_capture();
        eprintln!(
            "[megadaemon][PANIC] hilo={} panic={}\n{}",
            hilo, info, backtrace
        );
    }));
}

fn main() {
    install_panic_hook();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "cockpit" => {
            let port = args.get(2).and_then(|s| s.parse::<u16>().ok()).unwrap_or(3000);
            // v15 (W0): bucle autónomo de la voz.  Opt-in explícito
            // (MEGADAEMON_VOICE=1): 1 acción por tick, cooldown, presupuesto
            // por hora y NUNCA reinicios ni procesos nuevos.
            megadaemon_core::voice_runtime::spawn_autonomous_loop();
            if let Err(e) = cockpit_server::run_server(port) {
                eprintln!("Cockpit server error: {}", e);
                std::process::exit(1);
            }
        }

        "mcp-server" => {
            mcp_server::run_stdio_mcp_server();
        }

        // v16 — Capa de automatización (CLI de los 5 módulos nuevos).
        //   megadaemon-core automation <list|add|remove|enable|disable|poll|events|fire|journal>
        //   megadaemon-core cron       <list|add|remove|tick|plan|resume>
        //   megadaemon-core energy    <status|snapshot|profiles|apply|set_profile> [perfil]
        //   megadaemon-core comms     <status|consent|decide|handle|journal> [evento] [peer] [texto]
        //   megadaemon-core bridge    <detect|report|fire|inbox> [paquete] [accion]
        "automation" | "cron" | "energy" | "comms" | "bridge" => {
            let cmd = args[1].as_str();
            let action = args.get(2).cloned().unwrap_or_else(|| "list".to_string());
            let rest: Vec<String> = args.iter().skip(3).cloned().collect();
            let r: serde_json::Value = match cmd {
                "automation" => match action.as_str() {
                    "add" => {
                        // La regla llega como JSON en el último argumento.
                        let raw = rest.join(" ");
                        let rule: serde_json::Value = serde_json::from_str(&raw)
                            .unwrap_or_else(|e| serde_json::json!({"ok": false, "error": e.to_string()}));
                        match megadaemon_core::automation_runtime::automation_add(&rule) {
                            Ok(v) => v,
                            Err(e) => serde_json::json!({"ok": false, "error": e}),
                        }
                    }
                    "remove" => match megadaemon_core::automation_runtime::automation_remove(rest.first().map(|s| s.as_str()).unwrap_or("")) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "enable" => match megadaemon_core::automation_runtime::automation_toggle(rest.first().map(|s| s.as_str()).unwrap_or(""), true) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "disable" => match megadaemon_core::automation_runtime::automation_toggle(rest.first().map(|s| s.as_str()).unwrap_or(""), false) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "poll" => megadaemon_core::automation_runtime::automation_poll(false),
                    "events" => serde_json::json!({
                        "ok": true,
                        "events": megadaemon_core::automation_runtime::poll_events().iter().map(|e| serde_json::json!({
                            "kind": e.kind.as_str(), "key": e.key, "value": e.value, "ts": e.ts
                        })).collect::<Vec<_>>(),
                    }),
                    "fire" => {
                        let kind = rest.first().cloned().unwrap_or_default();
                        let key = rest.get(1).cloned().unwrap_or_default();
                        let text = rest.get(2).cloned().unwrap_or_default();
                        match megadaemon_core::automation_runtime::automation_fire(&kind, &key, None, &text, false) {
                            Ok(v) => v,
                            Err(e) => serde_json::json!({"ok": false, "error": e}),
                        }
                    }
                    "journal" => megadaemon_core::automation_runtime::automation_journal(50),
                    _ => megadaemon_core::automation_runtime::automation_list(),
                },
                "cron" => match action.as_str() {
                    "add" => {
                        let raw = rest.join(" ");
                        let job: serde_json::Value = serde_json::from_str(&raw)
                            .unwrap_or_else(|e| serde_json::json!({"ok": false, "error": e.to_string()}));
                        match megadaemon_core::automation_runtime::cron_add(&job) {
                            Ok(v) => v,
                            Err(e) => serde_json::json!({"ok": false, "error": e}),
                        }
                    }
                    "remove" => match megadaemon_core::automation_runtime::cron_remove(rest.first().map(|s| s.as_str()).unwrap_or("")) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "tick" => megadaemon_core::automation_runtime::cron_tick(false),
                    "plan" => megadaemon_core::automation_runtime::cron_plan(),
                    "resume" => megadaemon_core::automation_runtime::cron_resume(),
                    _ => megadaemon_core::automation_runtime::cron_list(),
                },
                "energy" => match action.as_str() {
                    "snapshot" => serde_json::json!({
                        "ok": true,
                        "raw": megadaemon_core::automation_runtime::probe_energy_text(),
                        "snapshot": megadaemon_core::energy_automation::snapshot_to_json(
                            &megadaemon_core::automation_runtime::energy_snapshot(),
                        ),
                    }),
                    "profiles" => megadaemon_core::automation_runtime::energy_profiles(),
                    "apply" => megadaemon_core::automation_runtime::energy_apply(rest.first().map(|s| s.as_str()).unwrap_or("")),
                    "set_profile" => match megadaemon_core::automation_runtime::energy_set_profile(rest.first().map(|s| s.as_str()).unwrap_or("")) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    _ => megadaemon_core::automation_runtime::energy_status(),
                },
                "comms" => match action.as_str() {
                    "decide" => match megadaemon_core::automation_runtime::comms_decide(
                        rest.first().map(|s| s.as_str()).unwrap_or(""),
                        rest.get(1).map(|s| s.as_str()).unwrap_or(""),
                        rest.get(2).map(|s| s.as_str()).unwrap_or(""),
                    ) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "handle" => match megadaemon_core::automation_runtime::comms_handle(
                        rest.first().map(|s| s.as_str()).unwrap_or(""),
                        rest.get(1).map(|s| s.as_str()).unwrap_or(""),
                        rest.get(2).map(|s| s.as_str()).unwrap_or(""),
                    ) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "journal" => megadaemon_core::automation_runtime::comms_journal(50),
                    _ => megadaemon_core::automation_runtime::comms_status(),
                },
                _ => match action.as_str() {
                    "report" => megadaemon_core::automation_runtime::bridge_report(),
                    "fire" => match megadaemon_core::automation_runtime::bridge_fire(
                        rest.first().map(|s| s.as_str()).unwrap_or(""),
                        rest.get(1).map(|s| s.as_str()).unwrap_or(""),
                        &[],
                    ) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "inbox" => megadaemon_core::automation_runtime::bridge_inbox(),
                    _ => megadaemon_core::automation_runtime::bridge_detect(),
                },
            };
            println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
        }

        // v15 (W0): la voz a mano (status | plan | act | whoami | catalog).
        // Útil para depurar el bucle autónomo sin esperar al siguiente tick.
        "voice" => {
            let action = args.get(2).cloned().unwrap_or_else(|| "status".to_string());
            let mut payload = serde_json::Map::new();
            payload.insert("action".into(), serde_json::Value::String(action.clone()));
            for a in args.iter().skip(3) {
                if let Some((k, v)) = a.split_once('=') {
                    let key = k.trim_start_matches("--").to_string();
                    match key.as_str() {
                        "llm" | "broadcast" => {
                            payload.insert(
                                key,
                                serde_json::Value::Bool(v.trim() == "1" || v.trim() == "true"),
                            );
                        }
                        "args" => {
                            let parsed: serde_json::Value =
                                serde_json::from_str(v).unwrap_or(serde_json::Value::Null);
                            payload.insert(key, parsed);
                        }
                        _ => {
                            payload.insert(key, serde_json::Value::String(v.to_string()));
                        }
                    }
                }
            }
            let r = megadaemon_core::voice_runtime::handle(&action, &serde_json::Value::Object(payload));
            println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
        }

        // v13.3 — Motor de sincronización de conversaciones (CLI).
        "sync" => {
            let services: Option<Vec<String>> = args.get(2).map(|s| {
                s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
            });
            let cookies = args.iter().any(|a| a == "--cookies");
            let r = megadaemon_core::sync::run_sync(services, cookies);
            println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
        }

        "conv-search" => {
            let q = args.get(2).cloned().unwrap_or_default();
            let svc = args.get(3).cloned();
            let r = megadaemon_core::sync::search(&q, svc.as_deref(), 20);
            println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
        }

        "conv-stats" => {
            let r = megadaemon_core::sync::status();
            println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
        }

        "chat" => {
            let model = args.get(2).map(|s| s.as_str());
            let prompt = if args.len() > 3 { args[3..].join(" ") } else { "Hello".into() };
            let res = providers::execute_chat(&prompt, model, None, None, 1024);
            println!("{}", serde_json::to_string_pretty(&res).unwrap());
        }

        "die-binwalk" => {
            let fp = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let rep = binwalk_die::analyze_binary_full(fp);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "cirx-ssa" => {
            let mas = args.get(2).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.75);
            let rep = cirx_rtl::run_reference_pipeline(mas, 10, 0.05, 3, 1, 0);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "mba" => {
            let expr = if args.len() > 2 { args[2..].join(" ") } else { "(x ^ y) + 2*(x & y)".into() };
            let rep = kaitai_imhex::simplify_mba_expression(&expr);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "kaitai" => {
            let fp = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let tmpl = args.get(3).map(|s| s.as_str()).unwrap_or("sega_m68k_vectors");
            let off = args.get(4).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            match kaitai_imhex::parse_struct_at_offset(fp, tmpl, off, None) {
                Ok(rep) => println!("{}", serde_json::to_string_pretty(&rep).unwrap()),
                Err(e) => { eprintln!("{}", e); std::process::exit(1); }
            }
        }

        "rizin" => {
            let fp = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let mode = args.get(3).map(|s| s.as_str()).unwrap_or("info");
            let arch = args.get(4).map(|s| s.as_str());
            let rep = frida_dbi::run_rizin_analysis(fp, mode, arch);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "frida" => {
            let mode = args.get(2).map(|s| s.as_str()).unwrap_or("attach");
            let module = args.get(3).map(|s| s.as_str()).unwrap_or("libtarget.so");
            let sym = args.get(4).map(|s| s.as_str()).unwrap_or("0x1000");
            let proc_n = args.get(5).map(|s| s.as_str()).unwrap_or("com.example.target");
            let rep = frida_dbi::generate_frida_script(mode, module, sym, proc_n);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "npu-cv" => {
            let rep = npu_cv::analyze_vla_frame_and_accel();
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "unifier" => {
            let rep = unifier::scan_unified_ecosystem();
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "rea" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            let query = args.get(3).map(|s| s.as_str()).unwrap_or("");
            let max_r = args.get(4).and_then(|s| s.parse::<usize>().ok()).unwrap_or(5);
            let rep = rea_assistant::analyze_ghidra_corpus(plat, query, max_r);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "cfg" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            let offset = args.get(3).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            let count = args.get(4).and_then(|s| s.parse::<usize>().ok()).unwrap_or(28);
            if let Some(rom) = rom_locator::resolve_binary(plat) {
                match cfg_lifter::lift_cfg(plat, &rom.platform_name, &rom.arch, &rom.path, offset, count) {
                    Ok(rep) => println!("{}", serde_json::to_string_pretty(&rep).unwrap()),
                    Err(e) => {
                        eprintln!("CFG error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Binary for platform '{}' not found", plat);
                std::process::exit(1);
            }
        }

        "bindiff" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            let rep = bindiff::run_cross_platform_bindiff(plat);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "hw-mmio" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            let (_, _, _, c_path, _) = rom_locator::platform_metadata(&plat.to_lowercase());
            let sample = fs::read_to_string(c_path)
                .map(|s| megadaemon_core::util_text::truncate_chars(&s, 65536))
                .unwrap_or_default();
            let rep = hw_mapper::analyze_text_for_mmio_and_structs(plat, &sample);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "crypto-scan" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            if let Some(rom) = rom_locator::resolve_binary(plat) {
                let rep = crypto_scan::scan_crypto_and_encodings(plat, &rom.path);
                println!("{}", serde_json::to_string_pretty(&rep).unwrap());
            } else {
                eprintln!("Binary for platform '{}' not found", plat);
                std::process::exit(1);
            }
        }

        "seere" => {
            if args.len() < 3 {
                eprintln!("Usage: megadaemon-core seere <query>");
                std::process::exit(1);
            }
            let mut query_parts: Vec<String> = Vec::new();
            let mut max_turns = 4usize;
            let mut env_in_prompt = false;
            // BUG v13.2 (HW-09): con "--max-turns 8", el "8" acababa en la
            // consulta del LLM (la bandera se saltaba pero su valor no).
            // Ahora se consume el valor de la bandera.
            let rest = &args[2..];
            let mut i = 0usize;
            while i < rest.len() {
                let a = &rest[i];
                if a == "--env-in-prompt" {
                    env_in_prompt = true;
                } else if a == "--max-turns" {
                    if let Some(v) = rest.get(i + 1) {
                        max_turns = v.parse().unwrap_or(4);
                        i += 1;
                    }
                } else if let Some(v) = a.strip_prefix("--max-turns=") {
                    max_turns = v.parse().unwrap_or(4);
                } else {
                    query_parts.push(a.clone());
                }
                i += 1;
            }
            let query = query_parts.join(" ");
            // v13.2 (S02): el guardián de taint bloquea la ejecución y la
            // delegación cuando el contexto viene de contenido no fiable
            // (web/fichero).  Escape hatch explícito del operador.
            let allow_tainted = env::var("SEERE_ALLOW_TAINTED")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let cfg = seere_v11::SeereConfig {
                max_turns,
                allow_tools: true,
                bin_mode: false,
                plan_mode: false,
                allow_exec: true,
                allow_external: false,
                env_in_prompt,
                re_opcodes: false,
                allow_tainted,
            };
            let res = seere_v11::run_seere(&query, &cfg);
            println!("{}", serde_json::to_string(&res).unwrap());
        }

        "power" => {
            let (st, pol) = power_policy::current_policy();
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "policy": pol.name, "max_turns": pol.max_turns,
                "prefer_local": pol.prefer_local, "allow_heavy": pol.allow_heavy,
                "max_tokens": pol.max_tokens, "reason": pol.reason,
                "charging": st.charging, "battery_pct": st.capacity_pct,
                "battery_temp_c": st.battery_temp_c, "cpu_temp_c": st.cpu_temp_c,
                "thermal_hot": st.thermal_hot, "system_power_save": st.system_power_save,
                "governor": st.governor
            })).unwrap());
        }
        "seere-prompt" => {
            let lang = if args.len() > 2 { args[2].clone() } else { "es".to_string() };
            println!("{}", seere_v11::sys_prompt("REACT", &lang));
        }
        "seere-grammar" => {
            println!("{}", seere_v11::build_act_grammar());
        }
        "seere-selftest" => match seere_v11::selftest() {
            Ok(msg) => println!("{}", msg),
            Err(e) => {
                eprintln!("SEERE v12 selftest failed: {}", e);
                std::process::exit(1);
            }
        },

        "npu-probe" => {
            // v15: `--vs4l` sondea el driver NPU con el metodo correcto
            // (ioctl SIN argumento => el driver devuelve el tamano que espera).
            if args.iter().any(|a| a == "--vs4l" || a == "-v") {
                let rep = npu_vs4l::probe();
                println!("{}", serde_json::to_string_pretty(&rep).unwrap());
            } else {
                let rep = npu_bridge::probe_exynos_npu();
                println!("{}", serde_json::to_string_pretty(&rep).unwrap());
            }
        }

        "csc-verify" => {
            // Verificador post-parche del CSC: la feature de grabacion nativa
            // esta realmente activa en el movil? (no "deberia estarlo").
            // `csc-verify --expect <valor>` exige un valor EXACTO (p.ej.
            // "RecordingAllowed" o "RecordingAllowedByMenu,disclaimer_latin").
            let expect = args
                .windows(2)
                .find(|w| w[0] == "--expect")
                .map(|w| w[1].to_string());
            let rep = csc_verify::verify_expecting(expect.as_deref());
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "locate-roms" => {
            let refresh = args.get(2).map(|s| s == "1" || s == "true").unwrap_or(false);
            let roms = rom_locator::locate_all(refresh);
            println!("{}", serde_json::to_string(&roms).unwrap());
        }

        "disasm" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core disasm <plat> <file> [offset] [count]");
                std::process::exit(1);
            }
            let plat = &args[2];
            let file_path = &args[3];
            let offset = args.get(4).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            let count = args.get(5).and_then(|s| s.parse::<usize>().ok()).unwrap_or(16);
            let (plat_name, arch, _, _, _) = rom_locator::platform_metadata(plat);
            match disasm::disassemble_file(plat, plat_name, arch, file_path, offset, count) {
                Ok(rep) => println!("{}", serde_json::to_string(&rep).unwrap()),
                Err(e) => {
                    eprintln!("Disasm error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "hex" => {
            if args.len() < 3 {
                eprintln!("Usage: megadaemon-core hex <file> [offset] [length]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let offset = args.get(3).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            let length = args.get(4).and_then(|s| s.parse::<usize>().ok()).unwrap_or(256);
            match disasm::inspect_hex(file_path, "ARM64/Retro", offset, length) {
                Ok(rep) => println!("{}", serde_json::to_string(&rep).unwrap()),
                Err(e) => {
                    eprintln!("Hex error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "deobfuscate" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core deobfuscate <plat> <file>");
                std::process::exit(1);
            }
            let rep = deobfuscate::deobfuscate_binary(&args[2], &args[3], 32);
            println!("{}", serde_json::to_string(&rep).unwrap());
        }

        "dumper" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core dumper <plat> <file>");
                std::process::exit(1);
            }
            let rep = deobfuscate::extract_assets_scan(&args[2], &args[3]);
            println!("{}", serde_json::to_string(&rep).unwrap());
        }

        "entropy" => {
            if args.len() < 3 {
                eprintln!("Usage: megadaemon-core entropy <file> [block_size]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let block_size = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1024);

            let mut f = match File::open(file_path) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Error opening file: {}", e);
                    std::process::exit(1);
                }
            };
            let mut data = Vec::new();
            if let Err(e) = f.read_to_end(&mut data) {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1);
            }

            let res = entropy::scan_entropy(&data, block_size);
            println!("{}", serde_json::to_string(&res).unwrap());
        }

        "pattern" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core pattern <file> <pattern_str> [max_matches]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let pat_str = &args[3];
            let max_matches = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(128);

            let mut f = match File::open(file_path) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Error opening file: {}", e);
                    std::process::exit(1);
                }
            };
            let mut data = Vec::new();
            if let Err(e) = f.read_to_end(&mut data) {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1);
            }

            match pattern::scan_pattern(&data, pat_str, max_matches) {
                Ok(res) => println!("{}", serde_json::to_string(&res).unwrap()),
                Err(err) => {
                    eprintln!("Pattern error: {}", err);
                    std::process::exit(1);
                }
            }
        }

        "pointers" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core pointers <file> <base_vaddr_hex> [is_be] [max]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let base_hex = args[3].trim_start_matches("0x");
            let base_vaddr = u32::from_str_radix(base_hex, 16).unwrap_or(0);
            let is_be = args.get(4).map(|s| s == "1" || s == "true").unwrap_or(false);
            let max_res = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(128);

            let mut f = match File::open(file_path) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Error opening file: {}", e);
                    std::process::exit(1);
                }
            };
            let mut data = Vec::new();
            if let Err(e) = f.read_to_end(&mut data) {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1);
            }

            let res = pointers::scan_pointers_32(&data, base_vaddr, is_be, max_res);
            println!("{}", serde_json::to_string(&res).unwrap());
        }

        "diff" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core diff <file_a> <file_b> [base_vaddr_hex] [max]");
                std::process::exit(1);
            }
            let file_a_path = &args[2];
            let file_b_path = &args[3];
            let base_hex = args.get(4).map(|s| s.trim_start_matches("0x")).unwrap_or("0");
            let base_vaddr = u32::from_str_radix(base_hex, 16).unwrap_or(0);
            let max_res = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(256);

            let mut f_a = match File::open(file_a_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error opening file_a: {}", e);
                    std::process::exit(1);
                }
            };
            let mut buf_a = Vec::new();
            f_a.read_to_end(&mut buf_a).unwrap();

            let mut f_b = match File::open(file_b_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error opening file_b: {}", e);
                    std::process::exit(1);
                }
            };
            let mut buf_b = Vec::new();
            f_b.read_to_end(&mut buf_b).unwrap();

            let res = snapshot::diff_snapshots(&buf_a, &buf_b, base_vaddr, max_res);
            println!("{}", serde_json::to_string(&res).unwrap());
        }

        "shm-status" => match shm::read_shm_status() {
            Ok(hdr) => println!("{}", serde_json::to_string(&hdr).unwrap()),
            Err(e) => {
                eprintln!("SHM Error: {}", e);
                std::process::exit(1);
            }
        },

        "shm-frame-bmp" => {
            // v13.2 (HW-08): la cabecera la escribe la app y puede ser
            // incoherente (1000x1000 en un fichero de 1 MB) -> el slice
            // reventaba con panic.  Ahora se usa la instantánea validada.
            let bmp_bytes = match shm::read_frame_snapshot() {
                Ok(snap) if snap.header.width > 0 && snap.header.height > 0 => shm::encode_bmp_24(
                    snap.header.width as u32,
                    snap.header.height as u32,
                    &snap.rgb,
                ),
                _ => shm::generate_tactical_hud_bmp(682, 448),
            };
            io::stdout().write_all(&bmp_bytes).unwrap();
        }

        cmd => {
            eprintln!("Unknown command: {}", cmd);
            print_usage();
            std::process::exit(1);
        }
    }
}
