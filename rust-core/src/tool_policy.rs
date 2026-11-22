//! Unified Tool Registry, Capability Policy Gate, and SQLite Audit Journal (PoC 1).
//!
//! Ensures every frontend (agent_coding, MCP, CLI, cockpit, voice) routes tool calls
//! through a single typed capability check and durable SQLite audit log.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityClass {
    ReadOnly,
    WorkspaceWrite,
    ProcessExec,
    Network,
    Secrets,
    ReverseEng,
    Eid,
}

impl CapabilityClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::WorkspaceWrite => "workspace_write",
            Self::ProcessExec => "process_exec",
            Self::Network => "network",
            Self::Secrets => "secrets",
            Self::ReverseEng => "reverse_eng",
            Self::Eid => "eid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PolicyDecision {
    Allow,
    Prompt { reason: String },
    Deny { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContext {
    pub caller: String,
    pub session_id: String,
    pub workspace_root: PathBuf,
    pub allow_outside_workspace: bool,
    pub auto_approve_writes: bool,
    pub auto_approve_exec: bool,
    pub allow_network: bool,
    pub allow_secrets: bool,
}

impl ToolContext {
    pub fn for_agent(workspace_root: PathBuf, session_id: impl Into<String>, allow_outside: bool) -> Self {
        Self {
            caller: "agent_coding".to_string(),
            session_id: session_id.into(),
            workspace_root,
            allow_outside_workspace: allow_outside,
            auto_approve_writes: true,
            auto_approve_exec: true,
            allow_network: false,
            allow_secrets: false,
        }
    }

    pub fn for_mcp(workspace_root: PathBuf) -> Self {
        Self {
            caller: "mcp".to_string(),
            session_id: "mcp".to_string(),
            workspace_root,
            allow_outside_workspace: true,
            auto_approve_writes: true,
            auto_approve_exec: true,
            allow_network: true,
            allow_secrets: false,
        }
    }
}

/// Classifies a tool (and optional MCP sub-tool) into its `CapabilityClass`.
pub fn classify_tool(tool_name: &str, args: &Value) -> CapabilityClass {
    match tool_name {
        "view_file" | "search_code" | "list_dir" | "ast_symbols" | "ast_check" => {
            CapabilityClass::ReadOnly
        }
        "replace_file_content" | "write_to_file" => CapabilityClass::WorkspaceWrite,
        "run_command" => CapabilityClass::ProcessExec,
        "megadaemon_tool" => {
            let inner = args.get("tool_name").and_then(Value::as_str).unwrap_or("");
            classify_mcp_tool(inner, args.get("arguments").unwrap_or(&Value::Null))
        }
        other => classify_mcp_tool(other, args),
    }
}

pub fn classify_mcp_tool(tool_name: &str, args: &Value) -> CapabilityClass {
    match tool_name {
        "capabilities" | "system_overview" | "repo_updates" => CapabilityClass::ReadOnly,
        "go_retool" | "die_binwalk" | "rizin_analyze" | "cirx_ssa" | "mba_simplify"
        | "re_inspect" => CapabilityClass::ReverseEng,
        "es_eid" => CapabilityClass::Eid,
        "drive_backup" => CapabilityClass::Network,
        "signing_vault" | "api_keys" => CapabilityClass::Secrets,
        "coding_agent" | "job_queue" => {
            let action = args.get("action").and_then(Value::as_str).unwrap_or("");
            if matches!(action, "list" | "status" | "get") {
                CapabilityClass::ReadOnly
            } else {
                CapabilityClass::WorkspaceWrite
            }
        }
        _ => CapabilityClass::ProcessExec,
    }
}

/// Evaluates `PolicyDecision` for a tool invocation without executing it.
pub fn evaluate_policy(ctx: &ToolContext, tool_name: &str, args: &Value) -> PolicyDecision {
    let cap = classify_tool(tool_name, args);

    if tool_name == "run_command" {
        let cmd = args.get("command").and_then(Value::as_str).unwrap_or("");
        let lower = cmd.to_ascii_lowercase();
        for blocked in ["rm -rf /", "mkfs.", ":(){:|:&};:", "dd if=/dev/zero of=/dev/"] {
            if lower.contains(blocked) {
                return PolicyDecision::Deny {
                    reason: format!("blocked destructive command pattern '{}'", blocked),
                };
            }
        }
    }

    match cap {
        CapabilityClass::ReadOnly | CapabilityClass::ReverseEng | CapabilityClass::Eid => {
            PolicyDecision::Allow
        }
        CapabilityClass::WorkspaceWrite => {
            if ctx.auto_approve_writes {
                PolicyDecision::Allow
            } else {
                PolicyDecision::Prompt {
                    reason: format!("{} requires workspace write approval", tool_name),
                }
            }
        }
        CapabilityClass::ProcessExec => {
            if ctx.auto_approve_exec {
                PolicyDecision::Allow
            } else {
                PolicyDecision::Prompt {
                    reason: format!("{} requires process execution approval", tool_name),
                }
            }
        }
        CapabilityClass::Network => {
            if ctx.allow_network {
                PolicyDecision::Allow
            } else {
                PolicyDecision::Prompt {
                    reason: format!("{} requires network access approval", tool_name),
                }
            }
        }
        CapabilityClass::Secrets => {
            if ctx.allow_secrets {
                PolicyDecision::Allow
            } else {
                PolicyDecision::Deny {
                    reason: format!("{} denied by default secrets policy", tool_name),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub id: i64,
    pub ts_epoch: i64,
    pub caller: String,
    pub session_id: String,
    pub tool_name: String,
    pub capability: String,
    pub decision: String,
    pub ok: bool,
    pub duration_ms: i64,
    pub summary: String,
}

/// Durable SQLite-backed audit journal for all tool invocations.
pub struct AuditJournal {
    db_path: PathBuf,
}

impl AuditJournal {
    pub fn open(db_path: &Path) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create audit db dir {}: {}", parent.display(), e))?;
        }
        let conn =
            Connection::open(db_path).map_err(|e| format!("open audit db: {}", e))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS tool_audit (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 ts_epoch INTEGER NOT NULL,
                 caller TEXT NOT NULL,
                 session_id TEXT NOT NULL,
                 tool_name TEXT NOT NULL,
                 capability TEXT NOT NULL,
                 decision TEXT NOT NULL,
                 ok INTEGER NOT NULL,
                 duration_ms INTEGER NOT NULL,
                 summary TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_tool_audit_ts ON tool_audit(ts_epoch DESC);",
        )
        .map_err(|e| format!("init tool_audit schema: {}", e))?;
        Ok(Self {
            db_path: db_path.to_path_buf(),
        })
    }

    pub fn record(
        &self,
        ctx: &ToolContext,
        tool_name: &str,
        cap: CapabilityClass,
        decision: &PolicyDecision,
        ok: bool,
        duration_ms: i64,
        summary: &str,
    ) -> Result<(), String> {
        let conn =
            Connection::open(&self.db_path).map_err(|e| format!("open audit db: {}", e))?;
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let dec_str = match decision {
            PolicyDecision::Allow => "allow",
            PolicyDecision::Prompt { .. } => "prompt",
            PolicyDecision::Deny { .. } => "deny",
        };
        let short_summary: String = summary.chars().take(240).collect();
        conn.execute(
            "INSERT INTO tool_audit (ts_epoch, caller, session_id, tool_name, capability, decision, ok, duration_ms, summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                ts,
                ctx.caller,
                ctx.session_id,
                tool_name,
                cap.as_str(),
                dec_str,
                if ok { 1i64 } else { 0i64 },
                duration_ms,
                short_summary,
            ],
        )
        .map_err(|e| format!("insert tool_audit: {}", e))?;
        Ok(())
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<AuditRecord>, String> {
        let conn =
            Connection::open(&self.db_path).map_err(|e| format!("open audit db: {}", e))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, ts_epoch, caller, session_id, tool_name, capability, decision, ok, duration_ms, summary
                 FROM tool_audit ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|e| format!("prepare tool_audit recent: {}", e))?;
        let rows = stmt
            .query_map(params![limit as i64], |r| {
                Ok(AuditRecord {
                    id: r.get(0)?,
                    ts_epoch: r.get(1)?,
                    caller: r.get(2)?,
                    session_id: r.get(3)?,
                    tool_name: r.get(4)?,
                    capability: r.get(5)?,
                    decision: r.get(6)?,
                    ok: r.get::<_, i64>(7)? != 0,
                    duration_ms: r.get(8)?,
                    summary: r.get(9)?,
                })
            })
            .map_err(|e| format!("query tool_audit: {}", e))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| format!("row tool_audit: {}", e))?);
        }
        Ok(out)
    }
}

/// Runs a tool through the unified `PolicyGate` and logs the outcome to `AuditJournal` (best-effort).
pub fn invoke_with_policy<F>(
    ctx: &ToolContext,
    tool_name: &str,
    args: &Value,
    audit_db: Option<&Path>,
    exec_fn: F,
) -> Result<String, String>
where
    F: FnOnce() -> Result<String, String>,
{
    let t0 = Instant::now();
    let cap = classify_tool(tool_name, args);
    let decision = evaluate_policy(ctx, tool_name, args);

    let res = match &decision {
        PolicyDecision::Allow => exec_fn(),
        PolicyDecision::Prompt { reason } => Err(format!("POLICY_PROMPT: {}", reason)),
        PolicyDecision::Deny { reason } => Err(format!("POLICY_DENY: {}", reason)),
    };

    let elapsed = t0.elapsed().as_millis() as i64;
    if let Some(db) = audit_db {
        if let Ok(journal) = AuditJournal::open(db) {
            let summary = match &res {
                Ok(s) => s.lines().next().unwrap_or("ok").to_string(),
                Err(e) => e.clone(),
            };
            let _ = journal.record(ctx, tool_name, cap, &decision, res.is_ok(), elapsed, &summary);
        }
    }
    res
}

pub fn policy_summary_json(ctx: &ToolContext, tool_name: &str, args: &Value) -> Value {
    let cap = classify_tool(tool_name, args);
    let decision = evaluate_policy(ctx, tool_name, args);
    json!({
        "tool_name": tool_name,
        "capability": cap,
        "decision": decision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_blocks_destructive_shell_and_secrets_by_default() {
        let ctx = ToolContext::for_agent(PathBuf::from("/tmp/ws"), "s1", false);
        assert!(matches!(
            evaluate_policy(&ctx, "run_command", &json!({"command": "rm -rf / --no-preserve-root"})),
            PolicyDecision::Deny { .. }
        ));
        assert!(matches!(
            evaluate_policy(&ctx, "megadaemon_tool", &json!({"tool_name": "api_keys"})),
            PolicyDecision::Deny { .. }
        ));
        assert_eq!(
            evaluate_policy(&ctx, "view_file", &json!({"path": "Cargo.toml"})),
            PolicyDecision::Allow
        );
    }

    #[test]
    fn audit_journal_persists_and_queries_records() {
        let dir = std::env::temp_dir().join(format!("mg_audit_{}", std::process::id()));
        let db = dir.join("audit.db");
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = ToolContext::for_agent(dir.clone(), "test-sess", false);

        let out = invoke_with_policy(&ctx, "view_file", &json!({"path": "a.rs"}), Some(&db), || {
            Ok("read 10 lines".to_string())
        })
        .unwrap();
        assert_eq!(out, "read 10 lines");

        let denied = invoke_with_policy(
            &ctx,
            "run_command",
            &json!({"command": "rm -rf /"}),
            Some(&db),
            || Ok("should not run".to_string()),
        );
        assert!(denied.is_err());

        let journal = AuditJournal::open(&db).unwrap();
        let rows = journal.recent(10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].decision, "deny");
        assert!(!rows[0].ok);
        assert_eq!(rows[1].decision, "allow");
        assert!(rows[1].ok);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
