//! Sandboxed Embedded Scripting Engine via Rhai (PoC 3).
//!
//! Executes agent/user automation scripts in-process with strict operation,
//! call-depth, and string/array size limits and zero ambient OS/filesystem access.

use rhai::{Dynamic, Engine};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptResult {
    pub ok: bool,
    pub value: String,
    pub logs: Vec<String>,
    pub error: Option<String>,
}

/// Runs a Rhai script inside a strict resource-bounded sandbox.
pub fn eval_sandboxed(script: &str, max_ops: u64) -> ScriptResult {
    let mut engine = Engine::new();
    engine.set_max_operations(max_ops.clamp(100, 500_000));
    engine.set_max_expr_depths(32, 16);
    engine.set_max_call_levels(16);
    engine.set_max_string_size(64 * 1024);
    engine.set_max_array_size(4096);
    engine.set_max_map_size(1024);

    let logs = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let logs_ref = logs.clone();
    engine.on_print(move |msg| {
        if let Ok(mut g) = logs_ref.lock() {
            if g.len() < 128 {
                g.push(msg.to_string());
            }
        }
    });

    // Safe, deterministic pure helpers
    engine.register_fn("len_bytes", |s: &str| -> i64 { s.len() as i64 });
    engine.register_fn("contains_str", |hay: &str, needle: &str| -> bool {
        hay.contains(needle)
    });

    match engine.eval::<Dynamic>(script) {
        Ok(val) => ScriptResult {
            ok: true,
            value: val.to_string(),
            logs: logs.lock().map(|g| g.clone()).unwrap_or_default(),
            error: None,
        },
        Err(e) => ScriptResult {
            ok: false,
            value: String::new(),
            logs: logs.lock().map(|g| g.clone()).unwrap_or_default(),
            error: Some(e.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rhai_sandbox_evaluates_and_enforces_op_limit() {
        let res = eval_sandboxed(r#"let x = 21; print("hi"); x * 2"#, 10_000);
        assert!(res.ok);
        assert_eq!(res.value, "42");
        assert_eq!(res.logs, vec!["hi"]);

        let runaway = eval_sandboxed(r#"loop { }"#, 500);
        assert!(!runaway.ok);
        assert!(runaway.error.unwrap().to_lowercase().contains("operations"));
    }
}
