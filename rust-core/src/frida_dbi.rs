//! MegaDaemon v13.0 Zenith — Rizin 0.9.1 Multi-OS Disasm/Decomp Bridge + Frida DBI Script Generator + Root Process Inspector
//! Eliminates JVM/Ghidra cold-start overhead on ARM64 mobile by integrating native C Rizin directly from Rust.

use crate::seere_v11;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FridaScriptResult {
    pub mode: String,
    pub target_module: String,
    pub target_symbol: String,
    pub offset_hex: String,
    pub script_js: String,
    pub launch_cmd: String,
    pub engine: String,
}

/// Executes native `rizin` (already installed at $PREFIX/bin/rizin) for fast multi-OS analysis
/// Supports ELF, PE/PE32+, Mach-O, DEX, ARM64, x86-64, MIPS, M68K, Z80, SuperH, DOS 16-bit.
pub fn run_rizin_analysis(file_path: &str, mode: &str, arch_override: Option<&str>) -> Value {
    if !Path::new(file_path).exists() {
        return json!({"status": "error", "message": format!("File not found: {}", file_path)});
    }
    let rz_cmd = match mode {
        "functions" => "aa;aflj",
        "imports" => "iij",
        "exports" => "iEj",
        "strings" => "izj",
        "sections" => "iSj",
        "disasm" => "pd 32",
        "info" => "ij",
        _ => "ij;iSj;aa;afl",
    };

    let mut cmd = Command::new("rizin");
    cmd.args(["-q", "-e", "scr.color=0"]);
    if let Some(a) = arch_override {
        if !a.trim().is_empty() && a != "auto" {
            cmd.args(["-a", a]);
        }
    }
    cmd.args(["-c", rz_cmd, file_path]);

    match cmd.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let parsed_json: Option<Value> = serde_json::from_str(&stdout).ok();
            json!({
                "status": if out.status.success() { "OK" } else { "WARN" },
                "file": file_path,
                "mode": mode,
                "rizin_cmd": rz_cmd,
                "data": parsed_json.unwrap_or_else(|| json!(stdout.chars().take(12000).collect::<String>())),
                "stderr": stderr.chars().take(600).collect::<String>(),
                "engine": "rizin-0.9.1-native-arm64-bridge"
            })
        }
        Err(e) => json!({
            "status": "error",
            "message": format!("Failed to invoke rizin: {}", e)
        }),
    }
}

/// Generates surgical Frida DBI instrumentation scripts (Interceptor.attach, NativeFunction, Stalker, Android JNI/Java)
/// strictly for software inspection, runtime telemetry, emulator debugging, and localization research.
pub fn generate_frida_script(
    mode: &str,
    module_name: &str,
    symbol_or_offset: &str,
    process_name: &str,
) -> FridaScriptResult {
    let mod_clean = if module_name.trim().is_empty() {
        "libtarget.so"
    } else {
        module_name.trim()
    };
    let sym_clean = if symbol_or_offset.trim().is_empty() {
        "0x1000"
    } else {
        symbol_or_offset.trim()
    };
    let proc_clean = if process_name.trim().is_empty() {
        "com.example.target"
    } else {
        process_name.trim()
    };

    let is_hex_offset = sym_clean.starts_with("0x") || sym_clean.starts_with("0X");

    let script_js = match mode {
        "stalker" => format!(
            r#"// MegaDaemon v13 — Frida Stalker Basic-Block Coverage & Instruction Tracer
'use strict';
const modName = "{mod_clean}";
const target = {target_expr};
if (!target) throw new Error("Target not resolved: " + modName);

console.log("[MegaDaemon Stalker] Hooking @ " + target);
Interceptor.attach(target, {{
  onEnter(args) {{
    this.tid = Process.getCurrentThreadId();
    this.blocks = 0;
    Stalker.follow(this.tid, {{
      events: {{ call: true, compile: true }},
      onReceive(events) {{
        const parsed = Stalker.parse(events, {{ annotate: false, stringify: false }});
        this.blocks += parsed.length;
      }}
    }});
  }},
  onLeave(retval) {{
    Stalker.unfollow(this.tid);
    Stalker.flush();
    console.log("[Stalker] Thread " + this.tid + " traced events=" + this.blocks + " retval=" + retval);
  }}
}});
"#,
            mod_clean = mod_clean,
            target_expr = if is_hex_offset {
                format!("Module.findBaseAddress(\"{}\").add({})", mod_clean, sym_clean)
            } else {
                format!("Module.findExportByName(\"{}\", \"{}\")", mod_clean, sym_clean)
            }
        ),
        "native_call" => format!(
            r#"// MegaDaemon v13 — Frida NativeFunction Isolated Routine Harness
'use strict';
const base = Module.findBaseAddress("{mod_clean}");
const addr = {target_expr};
if (!addr) throw new Error("Address not found");

// Signature: return_type, [arg_types]
const targetFn = new NativeFunction(addr, 'int', ['pointer', 'int']);
const probeBuf = Memory.alloc(64);
probeBuf.writeByteArray([0x53, 0x4E, 0x41, 0x54, 0x00, 0x00, 0x00, 0x00]);
const rc = targetFn(probeBuf, 8);
console.log("[NativeFunction] rc=" + rc);
console.log(hexdump(probeBuf, {{ length: 64, header: true, ansi: false }}));
"#,
            mod_clean = mod_clean,
            target_expr = if is_hex_offset {
                format!("base.add({})", sym_clean)
            } else {
                format!("Module.findExportByName(\"{}\", \"{}\")", mod_clean, sym_clean)
            }
        ),
        "android_jni" => format!(
            r#"// MegaDaemon v13 — Android ART / JNI & Native Library Load Tracer
'use strict';
Java.perform(() => {{
  const System = Java.use("java.lang.System");
  System.loadLibrary.implementation = function(libName) {{
    console.log("[JNI] System.loadLibrary('" + libName + "')");
    return this.loadLibrary(libName);
  }};
}});
const regNatives = Module.findExportByName("libart.so", "_ZN3art3JNI15RegisterNativesEP7_JNIEnvP7_jclassPK15JNINativeMethodi");
if (regNatives) {{
  Interceptor.attach(regNatives, {{
    onEnter(args) {{
      const count = args[3].toInt32();
      console.log("[ART RegisterNatives] methods=" + count + " in {proc_clean}");
    }}
  }});
}}
"#,
            proc_clean = proc_clean
        ),
        "tenet_mem" => format!(
            r#"// MegaDaemon v13 — Tenet-Style Memory Access & Register Snapshot Recorder
'use strict';
const target = {target_expr};
let step = 0;
Interceptor.attach(target, {{
  onEnter(args) {{
    step++;
    console.log(JSON.stringify({{
      step,
      pc: this.context.pc,
      sp: this.context.sp,
      arg0: args[0],
      arg1: args[1],
      arg2: args[2],
      backtrace: Thread.backtrace(this.context, Backtracer.ACCURATE).slice(0, 6).map(DebugSymbol.fromAddress)
    }}));
  }}
}});
"#,
            target_expr = if is_hex_offset {
                format!("Module.findBaseAddress(\"{}\").add({})", mod_clean, sym_clean)
            } else {
                format!("Module.findExportByName(\"{}\", \"{}\")", mod_clean, sym_clean)
            }
        ),
        _ => format!(
            r#"// MegaDaemon v13 — Frida Interceptor.attach Telemetry Hook
'use strict';
const target = {target_expr};
if (!target) {{
  console.error("[!] Symbol/Offset not found in {mod_clean}: {sym_clean}");
}} else {{
  console.log("[+] Attaching Interceptor @ " + target);
  Interceptor.attach(target, {{
    onEnter(args) {{
      console.log("[->] {sym_clean}(a0=" + args[0] + ", a1=" + args[1] + ", a2=" + args[2] + ")");
      try {{
        console.log(hexdump(args[0], {{ length: 48, header: true, ansi: false }}));
      }} catch (_) {{}}
    }},
    onLeave(retval) {{
      console.log("[<-] {sym_clean} => " + retval);
    }}
  }});
}}
"#,
            mod_clean = mod_clean,
            sym_clean = sym_clean,
            target_expr = if is_hex_offset {
                format!("Module.findBaseAddress(\"{}\").add({})", mod_clean, sym_clean)
            } else {
                format!("Module.findExportByName(\"{}\", \"{}\")", mod_clean, sym_clean)
            }
        ),
    };

    FridaScriptResult {
        mode: mode.to_string(),
        target_module: mod_clean.to_string(),
        target_symbol: sym_clean.to_string(),
        offset_hex: if is_hex_offset {
            sym_clean.to_string()
        } else {
            "EXPORT_RESOLVED".to_string()
        },
        script_js,
        launch_cmd: format!("frida -U -n {} -l hook.js", proc_clean),
        engine: "megadaemon-frida-dbi-v13-rust".to_string(),
    }
}

/// Lists live Android/Linux processes & optional `/proc/<pid>/maps` memory regions using Magisk root `su`
pub fn inspect_live_processes(filter: &str, pid_maps: Option<u32>) -> Value {
    let arsenal = seere_v11::Arsenal::new(false, true, true);
    if let Some(pid) = pid_maps {
        let cmd = format!("head -n 60 /proc/{}/maps 2>&1", pid);
        let maps = arsenal.exec_root(&cmd);
        return json!({
            "status": "OK",
            "pid": pid,
            "maps": maps,
            "engine": "rust-root-procfs"
        });
    }
    let clean_f: String = filter
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '.' || *c == '-')
        .collect();
    let cmd = if clean_f.is_empty() {
        "ps -A -o PID,USER,RSS,NAME | head -n 45".to_string()
    } else {
        format!("ps -A -o PID,USER,RSS,NAME | grep -i '{}' | head -n 45", clean_f)
    };
    let out = arsenal.exec_root(&cmd);
    json!({
        "status": "OK",
        "filter": clean_f,
        "processes": out,
        "engine": "rust-root-procfs"
    })
}
