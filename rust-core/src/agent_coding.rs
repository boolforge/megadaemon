//! MegaDaemon Agentic Coding Engine (Pure Rust, Multi-Turn ReAct)
//!
//! Modeled on Claude Code, OpenAI Codex, OpenCode, and Antigravity:
//! - Persistent multi-turn conversation state (JSONL) with sliding-window &
//!   tool-output compaction so context never overflows.
//! - Atomic coding primitives (`view_file`, `replace_file_content`,
//!   `write_to_file`, `search_code`, `list_dir`, `run_command`) with workspace
//!   path sandboxing and OpenAI-compatible JSON Schemas.
//! - Native OpenAI `/chat/completions` tool-calling transport (`ureq`) with
//!   fenced-JSON fallback for models/local SEERE that do not emit native
//!   `tool_calls`, plus a mockable `CodingBackend` trait for deterministic tests.
//! - Multi-turn coding loop with identical-failure loop detection and automatic
//!   provider failover (`providers::load_vault()` -> local SEERE v12).

use crate::providers::{self, ProviderEntry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Hard caps to protect memory, context window, and mobile CPU/RAM.
const VIEW_FILE_MAX_LINES: usize = 400;
const VIEW_FILE_MAX_BYTES: usize = 32 * 1024;
const CMD_STREAM_MAX_BYTES: usize = 16 * 1024;
const CMD_DEFAULT_TIMEOUT_SECS: u64 = 30;
const CMD_MAX_TIMEOUT_SECS: u64 = 120;
const SEARCH_MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const SEARCH_DEFAULT_RESULTS: usize = 50;
const SEARCH_HARD_MAX_RESULTS: usize = 200;
const LIST_DIR_MAX_ENTRIES: usize = 500;
const OLD_TOOL_TRUNCATE_HEAD: usize = 400;
const OLD_TOOL_TRUNCATE_TAIL: usize = 120;
const DEFAULT_MAX_TOKENS: u32 = 2048;

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "target",
    "node_modules",
    "dist",
    "build",
    "__pycache__",
];

// ============================================================================
// 1. Multi-Turn Conversation State & Session Persistence
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant_with_tools(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_calls,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn tool_result(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
            name: Some(name.into()),
        }
    }

    /// Converts this message into OpenAI `/chat/completions` wire format.
    pub fn to_openai_json(&self) -> Value {
        match self.role {
            Role::System => json!({
                "role": "system",
                "content": self.content,
            }),
            Role::User => json!({
                "role": "user",
                "content": self.content,
            }),
            Role::Assistant => {
                if self.tool_calls.is_empty() {
                    json!({
                        "role": "assistant",
                        "content": self.content,
                    })
                } else {
                    let calls: Vec<Value> = self
                        .tool_calls
                        .iter()
                        .map(|tc| {
                            let args_str = match &tc.arguments {
                                Value::String(s) => s.clone(),
                                other => serde_json::to_string(other)
                                    .unwrap_or_else(|_| "{}".to_string()),
                            };
                            json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": args_str,
                                }
                            })
                        })
                        .collect();
                    let content_val = if self.content.is_empty() {
                        Value::Null
                    } else {
                        Value::String(self.content.clone())
                    };
                    json!({
                        "role": "assistant",
                        "content": content_val,
                        "tool_calls": calls,
                    })
                }
            }
            Role::Tool => {
                let mut obj = json!({
                    "role": "tool",
                    "tool_call_id": self.tool_call_id.as_deref().unwrap_or("call_0"),
                    "content": self.content,
                });
                if let Some(n) = &self.name {
                    if let Some(map) = obj.as_object_mut() {
                        map.insert("name".to_string(), Value::String(n.clone()));
                    }
                }
                obj
            }
        }
    }

    fn approx_chars(&self) -> usize {
        let mut total = self.content.len() + 24;
        for tc in &self.tool_calls {
            total += tc.id.len() + tc.name.len() + tc.arguments.to_string().len() + 16;
        }
        if let Some(id) = &self.tool_call_id {
            total += id.len();
        }
        if let Some(n) = &self.name {
            total += n.len();
        }
        total
    }
}

/// JSONL-backed session persistence and context-window manager.
#[derive(Debug, Clone)]
pub struct SessionStore {
    session_id: String,
    path: PathBuf,
    messages: Vec<ChatMessage>,
}

impl SessionStore {
    /// Creates a store rooted at `<state_dir>/coding_sessions/<session_id>.jsonl`.
    pub fn new(session_id: impl Into<String>) -> Self {
        let base = PathBuf::from(crate::cockpit_server::state_dir()).join("coding_sessions");
        Self::with_base_dir(base, session_id)
    }

    /// Creates a store rooted at a custom directory (useful for tests or isolated workspaces).
    pub fn with_base_dir(base_dir: impl AsRef<Path>, session_id: impl Into<String>) -> Self {
        let raw_id = session_id.into();
        let clean_id = sanitize_session_id(&raw_id);
        let path = base_dir.as_ref().join(format!("{}.jsonl", clean_id));
        Self {
            session_id: clean_id,
            path,
            messages: Vec::new(),
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn messages_mut(&mut self) -> &mut Vec<ChatMessage> {
        &mut self.messages
    }

    pub fn total_chars(&self) -> usize {
        self.messages.iter().map(ChatMessage::approx_chars).sum()
    }

    /// Appends a message in memory and appends one JSONL line to disk.
    pub fn push_and_persist(&mut self, msg: ChatMessage) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create session dir {}: {}", parent.display(), e))?;
        }
        let line = serde_json::to_string(&msg)
            .map_err(|e| format!("failed to serialize chat message: {}", e))?;
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("failed to open session {}: {}", self.path.display(), e))?;
        writeln!(f, "{}", line)
            .map_err(|e| format!("failed to append session {}: {}", self.path.display(), e))?;
        self.messages.push(msg);
        Ok(())
    }

    /// Atomically rewrites the entire session JSONL file (e.g. after compaction).
    pub fn save_all(&self) -> Result<(), String> {
        let mut buf = String::new();
        for msg in &self.messages {
            let line = serde_json::to_string(msg)
                .map_err(|e| format!("failed to serialize message: {}", e))?;
            buf.push_str(&line);
            buf.push('\n');
        }
        atomic_write(&self.path, buf.as_bytes())
    }

    /// Loads messages from the JSONL file on disk, ignoring blank or corrupt lines.
    pub fn reload(&mut self) -> Result<usize, String> {
        self.messages.clear();
        if !self.path.exists() {
            return Ok(0);
        }
        let raw = fs::read_to_string(&self.path)
            .map_err(|e| format!("failed to read session {}: {}", self.path.display(), e))?;
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(msg) = serde_json::from_str::<ChatMessage>(trimmed) {
                self.messages.push(msg);
            }
        }
        Ok(self.messages.len())
    }

    /// Sliding-window & tool-output compaction so context never overflows `max_chars`.
    ///
    /// Preserves:
    /// 1. Leading `System` prompt(s).
    /// 2. First `User` goal message.
    /// 3. Most recent turns (never splitting a `Tool` result from its `Assistant` tool_call).
    ///
    /// Returns `true` if compaction modified the message buffer.
    pub fn compact_if_needed(&mut self, max_chars: usize) -> bool {
        if max_chars == 0 || self.total_chars() <= max_chars {
            return false;
        }

        let mut modified = false;
        let len = self.messages.len();
        let protect_tail = len.saturating_sub(4);

        // Pass 1: truncate oversized old tool outputs (keeping recent 4 messages intact).
        for msg in self.messages.iter_mut().take(protect_tail) {
            if msg.role == Role::Tool
                && msg.content.len() > OLD_TOOL_TRUNCATE_HEAD + OLD_TOOL_TRUNCATE_TAIL + 64
            {
                let orig_len = msg.content.len();
                let head = truncate_utf8_prefix(&msg.content, OLD_TOOL_TRUNCATE_HEAD);
                let tail = truncate_utf8_suffix(&msg.content, OLD_TOOL_TRUNCATE_TAIL);
                let omitted = orig_len.saturating_sub(head.len() + tail.len());
                msg.content = format!(
                    "{}\n...[compacted {} chars of earlier tool output]...\n{}",
                    head, omitted, tail
                );
                modified = true;
            }
        }

        if self.total_chars() <= max_chars {
            return modified;
        }

        // Pass 2: sliding-window summary compaction preserving system + first user + recent tail.
        let mut head_end = 0usize;
        while head_end < self.messages.len() && self.messages[head_end].role == Role::System {
            head_end += 1;
        }
        if head_end < self.messages.len() && self.messages[head_end].role == Role::User {
            head_end += 1;
        }

        if self.messages.len() <= head_end + 2 {
            // Even with minimal turns, hard-trim any huge tool output in the remaining messages.
            for msg in self.messages.iter_mut().skip(head_end) {
                if msg.role == Role::Tool && msg.content.len() > max_chars / 2 {
                    let cap = (max_chars / 2).max(256);
                    let cut = truncate_utf8_prefix(&msg.content, cap);
                    msg.content = format!("{}\n...[truncated to fit context budget]", cut);
                    modified = true;
                }
            }
            return modified;
        }

        let head_chars: usize = self.messages[..head_end]
            .iter()
            .map(ChatMessage::approx_chars)
            .sum();
        let summary_reserve = 512usize;
        let tail_budget = max_chars.saturating_sub(head_chars + summary_reserve);

        // Walk backwards to choose as many recent messages as fit within tail_budget.
        let mut tail_start = self.messages.len();
        let mut acc = 0usize;
        for idx in (head_end..self.messages.len()).rev() {
            let sz = self.messages[idx].approx_chars();
            if tail_start < self.messages.len() && acc + sz > tail_budget {
                break;
            }
            acc += sz;
            tail_start = idx;
        }

        // Never leave orphaned Tool messages at the start of the kept tail:
        // pull tail_start backward to include the Assistant message that issued the tool_calls,
        // or if that would cross head_end, advance tail_start past the orphaned Tool messages.
        while tail_start < self.messages.len() && self.messages[tail_start].role == Role::Tool {
            if tail_start > head_end {
                tail_start -= 1;
            } else {
                while tail_start < self.messages.len()
                    && self.messages[tail_start].role == Role::Tool
                {
                    tail_start += 1;
                }
                break;
            }
        }

        if tail_start <= head_end {
            return modified;
        }

        let dropped = &self.messages[head_end..tail_start];
        let mut tools_seen: Vec<String> = Vec::new();
        for m in dropped {
            for tc in &m.tool_calls {
                if !tools_seen.contains(&tc.name) {
                    tools_seen.push(tc.name.clone());
                }
            }
            if let Some(n) = &m.name {
                if !tools_seen.contains(n) {
                    tools_seen.push(n.clone());
                }
            }
        }

        let summary_text = format!(
            "[Context compacted: {} earlier turn(s) summarized to stay within {} char budget. Tools used earlier: {}. Continue from the recent state below.]",
            dropped.len(),
            max_chars,
            if tools_seen.is_empty() {
                "none".to_string()
            } else {
                tools_seen.join(", ")
            }
        );

        let mut compacted = Vec::with_capacity(head_end + 1 + (self.messages.len() - tail_start));
        compacted.extend_from_slice(&self.messages[..head_end]);
        compacted.push(ChatMessage::system(summary_text));
        compacted.extend_from_slice(&self.messages[tail_start..]);
        self.messages = compacted;
        true
    }
}

fn sanitize_session_id(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        format!("session_{}", now_unix_secs())
    } else {
        cleaned
    }
}

// ============================================================================
// 2. Atomic Coding Primitives & OpenAI-Compatible Tool Schemas
// ============================================================================

/// Returns the OpenAI `/chat/completions` `tools` array for the coding agent.
pub fn tool_definitions() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "view_file",
                "description": "Read a text file with 1-indexed line numbers (capped at 400 lines / 32KB per call). Always read relevant lines before editing.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path relative to workspace root (or absolute if allowed)."
                        },
                        "start_line": {
                            "type": "integer",
                            "description": "Optional 1-indexed start line (inclusive)."
                        },
                        "end_line": {
                            "type": "integer",
                            "description": "Optional 1-indexed end line (inclusive)."
                        }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "replace_file_content",
                "description": "Replace an exact substring in a file atomically. Fails safely if 0 matches or >1 matches unless allow_multiple is true. Returns a unified-style diff summary.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Target file path."
                        },
                        "target_content": {
                            "type": "string",
                            "description": "Exact existing substring to replace."
                        },
                        "replacement_content": {
                            "type": "string",
                            "description": "New content to substitute."
                        },
                        "allow_multiple": {
                            "type": "boolean",
                            "description": "If true, replace all occurrences. Default false."
                        }
                    },
                    "required": ["path", "target_content", "replacement_content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_to_file",
                "description": "Create or overwrite a file atomically (temp file + rename), creating parent directories if needed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Destination file path."
                        },
                        "content": {
                            "type": "string",
                            "description": "Complete text content to write."
                        },
                        "overwrite": {
                            "type": "boolean",
                            "description": "Must be true to overwrite an existing file. Default false."
                        }
                    },
                    "required": ["path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "search_code",
                "description": "Fast recursive substring search across text files in the workspace (skips .git, target, node_modules). Returns file:line: snippet matches.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Text pattern to search for."
                        },
                        "path": {
                            "type": "string",
                            "description": "Optional subdirectory or file to scope the search (default workspace root)."
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Maximum number of matches to return (default 50, max 200)."
                        },
                        "case_insensitive": {
                            "type": "boolean",
                            "description": "Case-insensitive matching if true (default false)."
                        }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_dir",
                "description": "List entries in a directory with type (dir/file) and byte size.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Optional directory path (default workspace root)."
                        }
                    },
                    "required": []
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "run_command",
                "description": "Execute a shell command via sh -c with timeout protection and 16KB stdout/stderr cap. Returns exit code, stdout, and stderr.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Shell command line to run."
                        },
                        "cwd": {
                            "type": "string",
                            "description": "Optional working directory (default workspace root)."
                        },
                        "timeout_secs": {
                            "type": "integer",
                            "description": "Optional timeout in seconds (default 30, max 120)."
                        }
                    },
                    "required": ["command"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "ast_symbols",
                "description": "Parse a Rust (.rs) source file with Tree-sitter to extract exact symbol ranges (fn, struct, enum, trait, impl, mod) and syntax errors.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to .rs file."
                        }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "rhai_eval",
                "description": "Evaluate a sandboxed Rhai script in-process with strict operation limits and zero ambient OS/filesystem access.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "script": {
                            "type": "string",
                            "description": "Rhai script source."
                        },
                        "max_ops": {
                            "type": "integer",
                            "description": "Max operations limit (100..500000, default 50000)."
                        }
                    },
                    "required": ["script"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "megadaemon_tool",
                "description": "Invoke any built-in MegaDaemon MCP tool natively in-process (e.g. daad_harvester, go_retool, drive_backup, clipboard, repo_updates, github, apk_inventory, etc.).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "tool_name": {
                            "type": "string",
                            "description": "Exact MegaDaemon MCP tool name (e.g. 'go_retool', 'daad_harvester', 'drive_backup')."
                        },
                        "arguments": {
                            "type": "object",
                            "description": "JSON arguments object forwarded to the tool."
                        }
                    },
                    "required": ["tool_name"]
                }
            }
        }
    ])
}

/// Resolves `raw_path` relative to `workspace_root` with symlink-aware sandboxing.
///
/// When `allow_outside` is `false`, any path resolving outside `workspace_root`
/// (via `..`, absolute paths, or symlinks) is rejected.
pub fn resolve_safe_path(
    workspace_root: &Path,
    raw_path: &str,
    allow_outside: bool,
) -> Result<PathBuf, String> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err("path must not be empty".to_string());
    }
    if trimmed.contains('\0') {
        return Err("path contains null byte".to_string());
    }

    let canon_root = fs::canonicalize(workspace_root)
        .unwrap_or_else(|_| lexical_normalize(workspace_root));

    let candidate = {
        let p = Path::new(trimmed);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            canon_root.join(p)
        }
    };

    let normalized = lexical_normalize(&candidate);
    let resolved = canonicalize_with_nonexistent_tail(&normalized);

    if !allow_outside && !resolved.starts_with(&canon_root) {
        return Err(format!(
            "path '{}' resolves outside workspace root '{}'",
            raw_path,
            canon_root.display()
        ));
    }

    Ok(resolved)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(comp.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            Component::Normal(seg) => out.push(seg),
        }
    }
    out
}

/// Canonicalizes the longest existing ancestor and re-appends any non-existent
/// trailing components so symlink escapes are caught even for new files.
fn canonicalize_with_nonexistent_tail(path: &Path) -> PathBuf {
    if let Ok(c) = fs::canonicalize(path) {
        return c;
    }
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = path;
    while let Some(parent) = cur.parent() {
        if let Some(name) = cur.file_name() {
            missing.push(name.to_os_string());
        }
        if let Ok(canon_parent) = fs::canonicalize(parent) {
            let mut full = canon_parent;
            for seg in missing.into_iter().rev() {
                full.push(seg);
            }
            return full;
        }
        cur = parent;
    }
    path.to_path_buf()
}

/// Writes `bytes` atomically to `target` via sibling temp file + `fs::rename`.
fn atomic_write(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("target path '{}' has no parent directory", target.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create directory '{}': {}", parent.display(), e))?;

    let fname = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_name = format!(".{}.tmp.{}.{}", fname, std::process::id(), nanos);
    let tmp_path = parent.join(tmp_name);

    let write_res = (|| -> Result<(), String> {
        let mut f = fs::File::create(&tmp_path)
            .map_err(|e| format!("failed to create temp file '{}': {}", tmp_path.display(), e))?;
        f.write_all(bytes)
            .map_err(|e| format!("failed to write temp file '{}': {}", tmp_path.display(), e))?;
        let _ = f.sync_all();
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }

    if let Err(e) = fs::rename(&tmp_path, target) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "failed to rename '{}' to '{}': {}",
            tmp_path.display(),
            target.display(),
            e
        ));
    }

    Ok(())
}

/// 1-indexed windowed file reader (capped at 400 lines / 32KB).
pub fn tool_view_file(
    workspace: &Path,
    allow_outside: bool,
    args: &Value,
) -> Result<String, String> {
    let raw_path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing required string argument 'path'".to_string())?;
    let path = resolve_safe_path(workspace, raw_path, allow_outside)?;

    let bytes = fs::read(&path)
        .map_err(|e| format!("failed to read '{}': {}", path.display(), e))?;
    let probe_len = bytes.len().min(4096);
    if bytes[..probe_len].contains(&0) {
        return Err(format!(
            "'{}' appears to be a binary file ({} bytes)",
            raw_path,
            bytes.len()
        ));
    }

    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len();

    if total_lines == 0 {
        return Ok(format!("File: {} (empty, 0 lines)", raw_path));
    }

    let start_line = args
        .get("start_line")
        .and_then(Value::as_u64)
        .map(|n| n.max(1) as usize)
        .unwrap_or(1);

    if start_line > total_lines {
        return Err(format!(
            "start_line {} exceeds total line count {} in '{}'",
            start_line, total_lines, raw_path
        ));
    }

    let requested_end = args
        .get("end_line")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(total_lines);

    if requested_end < start_line {
        return Err(format!(
            "end_line {} is less than start_line {}",
            requested_end, start_line
        ));
    }

    let max_end = (start_line + VIEW_FILE_MAX_LINES - 1).min(total_lines);
    let effective_end = requested_end.min(max_end);

    let mut body = String::new();
    let mut last_included = start_line.saturating_sub(1);
    let mut byte_capped = false;

    for idx in (start_line - 1)..effective_end {
        let formatted = format!("{}: {}\n", idx + 1, lines[idx]);
        if body.len() + formatted.len() > VIEW_FILE_MAX_BYTES && !body.is_empty() {
            byte_capped = true;
            break;
        }
        body.push_str(&formatted);
        last_included = idx + 1;
    }

    let mut out = format!(
        "File: {} (showing lines {}-{} of {})\n{}",
        raw_path, start_line, last_included, total_lines, body
    );

    if byte_capped || last_included < requested_end.min(total_lines) {
        out.push_str(&format!(
            "[truncated at line {}; call view_file with start_line={} to continue]\n",
            last_included,
            last_included + 1
        ));
    }

    Ok(out)
}

/// Exact substring replacement with uniqueness check, atomic write, and unified diff summary.
pub fn tool_replace_file_content(
    workspace: &Path,
    allow_outside: bool,
    args: &Value,
) -> Result<String, String> {
    let raw_path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing required string argument 'path'".to_string())?;
    let target = args
        .get("target_content")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing required string argument 'target_content'".to_string())?;
    let replacement = args
        .get("replacement_content")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing required string argument 'replacement_content'".to_string())?;
    let allow_multiple = args
        .get("allow_multiple")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if target.is_empty() {
        return Err("target_content must not be empty".to_string());
    }

    let path = resolve_safe_path(workspace, raw_path, allow_outside)?;
    let original = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read '{}': {}", path.display(), e))?;

    let match_count = original.matches(target).count();
    if match_count == 0 {
        return Err(format!(
            "target_content not found in '{}' (0 matches)",
            raw_path
        ));
    }
    if match_count > 1 && !allow_multiple {
        return Err(format!(
            "target_content matched {} times in '{}' (ambiguous); include more surrounding context or set allow_multiple=true",
            match_count, raw_path
        ));
    }

    let updated = if allow_multiple {
        original.replace(target, replacement)
    } else {
        original.replacen(target, replacement, 1)
    };

    atomic_write(&path, updated.as_bytes())?;

    let diff = build_concise_diff(raw_path, &original, &updated, match_count);
    Ok(diff)
}

fn build_concise_diff(path: &str, original: &str, updated: &str, count: usize) -> String {
    let diff = similar::TextDiff::from_lines(original, updated);
    let unified = diff
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{}", path), &format!("b/{}", path))
        .to_string();
    let mut lines: Vec<&str> = unified.lines().collect();
    let omitted = lines.len().saturating_sub(80);
    if omitted > 0 {
        lines.truncate(80);
    }
    let mut out = format!("Replaced {} occurrence(s) in {}\n{}", count, path, lines.join("\n"));
    if omitted > 0 {
        out.push_str(&format!("\n...[{} more diff lines omitted]", omitted));
    }
    out
}

/// Creates or overwrites a file atomically, creating parent directories as needed.
pub fn tool_write_to_file(
    workspace: &Path,
    allow_outside: bool,
    args: &Value,
) -> Result<String, String> {
    let raw_path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing required string argument 'path'".to_string())?;
    let content = args
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing required string argument 'content'".to_string())?;
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let path = resolve_safe_path(workspace, raw_path, allow_outside)?;
    if path.exists() && !overwrite {
        return Err(format!(
            "file '{}' already exists; set overwrite=true to replace it",
            raw_path
        ));
    }

    atomic_write(&path, content.as_bytes())?;
    Ok(format!(
        "Wrote {} bytes ({} lines) to {}",
        content.len(),
        content.lines().count(),
        raw_path
    ))
}

/// Ripgrep-powered recursive code search (`ignore` + `grep-searcher` + `grep-regex`)
/// honoring `.gitignore`, skipping binary files, and streaming bounded results.
pub fn tool_search_code(
    workspace: &Path,
    allow_outside: bool,
    args: &Value,
) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing required string argument 'query'".to_string())?;
    if query.is_empty() {
        return Err("search query must not be empty".to_string());
    }

    let raw_scope = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let scope_path = resolve_safe_path(workspace, raw_scope, allow_outside)?;
    let max_results = args
        .get("max_results")
        .and_then(Value::as_u64)
        .map(|n| (n as usize).clamp(1, SEARCH_HARD_MAX_RESULTS))
        .unwrap_or(SEARCH_DEFAULT_RESULTS);
    let case_insensitive = args
        .get("case_insensitive")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let matcher = grep_regex::RegexMatcherBuilder::new()
        .case_insensitive(case_insensitive)
        .fixed_strings(true)
        .build(query)
        .map_err(|e| format!("invalid search pattern: {}", e))?;

    let mut searcher = grep_searcher::SearcherBuilder::new()
        .binary_detection(grep_searcher::BinaryDetection::quit(b'\x00'))
        .line_number(true)
        .build();

    let canon_root = fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    let mut matches: Vec<String> = Vec::new();
    let mut truncated = false;

    let walker = ignore::WalkBuilder::new(&scope_path)
        .hidden(false)
        .git_ignore(true)
        .max_filesize(Some(SEARCH_MAX_FILE_BYTES))
        .filter_entry(|entry| {
            let name = entry.file_name().to_str().unwrap_or("");
            !SKIP_DIRS.contains(&name)
        })
        .build();

    for entry in walker.flatten() {
        if matches.len() >= max_results {
            truncated = true;
            break;
        }
        let path = entry.path();
        if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            continue;
        }
        let display_path = path
            .strip_prefix(&canon_root)
            .unwrap_or(path)
            .display()
            .to_string();

        let _ = searcher.search_path(
            &matcher,
            path,
            grep_searcher::sinks::UTF8(|line_num, line| {
                let snippet = truncate_utf8_prefix(line.trim(), 200);
                matches.push(format!("{}:{}: {}", display_path, line_num, snippet));
                if matches.len() >= max_results {
                    truncated = true;
                    Ok(false)
                } else {
                    Ok(true)
                }
            }),
        );
    }

    if matches.is_empty() {
        return Ok(format!("No matches found for '{}'", query));
    }

    let mut out = matches.join("\n");
    if truncated {
        out.push_str(&format!("\n[capped at {} results]", max_results));
    }
    Ok(out)
}

/// Lists directory entries with type (`dir`/`file`) and byte size.
pub fn tool_list_dir(
    workspace: &Path,
    allow_outside: bool,
    args: &Value,
) -> Result<String, String> {
    let raw_path = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let dir_path = resolve_safe_path(workspace, raw_path, allow_outside)?;
    let rd = fs::read_dir(&dir_path)
        .map_err(|e| format!("failed to list '{}': {}", dir_path.display(), e))?;

    let mut dirs: Vec<String> = Vec::new();
    let mut files: Vec<(String, u64)> = Vec::new();

    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata();
        match meta {
            Ok(m) if m.is_dir() => dirs.push(name),
            Ok(m) => files.push((name, m.len())),
            Err(_) => files.push((name, 0)),
        }
    }

    dirs.sort();
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut lines: Vec<String> = Vec::new();
    for d in &dirs {
        if lines.len() >= LIST_DIR_MAX_ENTRIES {
            break;
        }
        lines.push(format!("dir  {}/", d));
    }
    for (f, sz) in &files {
        if lines.len() >= LIST_DIR_MAX_ENTRIES {
            break;
        }
        lines.push(format!("file {} ({} B)", f, sz));
    }

    if lines.is_empty() {
        return Ok(format!("Directory '{}' is empty", raw_path));
    }

    let total = dirs.len() + files.len();
    let mut out = format!("Directory: {} ({} entries)\n{}", raw_path, total, lines.join("\n"));
    if total > LIST_DIR_MAX_ENTRIES {
        out.push_str(&format!("\n[showing first {} of {} entries]", LIST_DIR_MAX_ENTRIES, total));
    }
    Ok(out)
}

/// Executes a command via `sh -c` with timeout protection and 16KB stdout/stderr caps.
pub fn tool_run_command(
    workspace: &Path,
    allow_outside: bool,
    args: &Value,
) -> Result<String, String> {
    let cmd = args
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing required string argument 'command'".to_string())?;
    if cmd.trim().is_empty() {
        return Err("command must not be empty".to_string());
    }

    let raw_cwd = args.get("cwd").and_then(Value::as_str).unwrap_or(".");
    let cwd = resolve_safe_path(workspace, raw_cwd, allow_outside)?;
    let timeout_secs = args
        .get("timeout_secs")
        .and_then(Value::as_u64)
        .map(|s| s.clamp(1, CMD_MAX_TIMEOUT_SECS))
        .unwrap_or(CMD_DEFAULT_TIMEOUT_SECS);

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn shell command: {}", e))?;

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let t_out = thread::spawn(move || read_capped_stream(stdout_pipe, CMD_STREAM_MAX_BYTES));
    let t_err = thread::spawn(move || read_capped_stream(stderr_pipe, CMD_STREAM_MAX_BYTES));

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut timed_out = false;
    let exit_code: i32 = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(-1),
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    let _ = child.kill();
                    let _ = child.wait();
                    break -1;
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("failed waiting on command: {}", e));
            }
        }
    };

    let (stdout_str, out_trunc) = t_out.join().unwrap_or_else(|_| (String::new(), false));
    let (stderr_str, err_trunc) = t_err.join().unwrap_or_else(|_| (String::new(), false));

    let mut report = format!("exit_code: {}\n", exit_code);
    if timed_out {
        report.push_str(&format!("timed_out: true (limit {}s)\n", timeout_secs));
    }
    report.push_str("--- stdout ---\n");
    report.push_str(&stdout_str);
    if out_trunc {
        report.push_str("\n[stdout capped at 16KB]");
    }
    if !stderr_str.is_empty() {
        report.push_str("\n--- stderr ---\n");
        report.push_str(&stderr_str);
        if err_trunc {
            report.push_str("\n[stderr capped at 16KB]");
        }
    }
    Ok(report)
}

fn read_capped_stream<R: Read>(stream: Option<R>, cap: usize) -> (String, bool) {
    let Some(mut r) = stream else {
        return (String::new(), false);
    };
    let mut collected = Vec::new();
    let mut buf = [0u8; 4096];
    let mut truncated = false;

    loop {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if collected.len() < cap {
                    let take = n.min(cap - collected.len());
                    collected.extend_from_slice(&buf[..take]);
                    if take < n {
                        truncated = true;
                    }
                } else {
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }

    (String::from_utf8_lossy(&collected).to_string(), truncated)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecResult {
    pub ok: bool,
    pub output: String,
}

/// Dispatches a tool call by name against the workspace, gated by `tool_policy` and audited in SQLite.
pub fn dispatch_tool(
    name: &str,
    args: &Value,
    workspace: &Path,
    allow_outside: bool,
) -> ToolExecResult {
    let ctx = crate::tool_policy::ToolContext::for_agent(
        workspace.to_path_buf(),
        "agent_coding",
        allow_outside,
    );
    let audit_db = Path::new(&crate::cockpit_server::state_dir()).join("megadaemon_audit.db");

    let res = crate::tool_policy::invoke_with_policy(&ctx, name, args, Some(&audit_db), || match name {
        "view_file" => tool_view_file(workspace, allow_outside, args),
        "replace_file_content" => {
            let mut out = tool_replace_file_content(workspace, allow_outside, args)?;
            if let Some(raw_path) = args.get("path").and_then(Value::as_str) {
                if raw_path.ends_with(".rs") {
                    if let Ok(p) = resolve_safe_path(workspace, raw_path, allow_outside) {
                        if let Ok(ast) = crate::code_ast::analyze_file(&p) {
                            if ast.ok {
                                out.push_str(&format!("\n[AST CHECK: OK ({} symbols)]", ast.symbol_count));
                            } else {
                                out.push_str(&format!("\n[AST SYNTAX WARNING: {} error(s): {:?}]", ast.error_count, ast.errors));
                            }
                        }
                    }
                }
            }
            Ok(out)
        }
        "write_to_file" => {
            let mut out = tool_write_to_file(workspace, allow_outside, args)?;
            if let Some(raw_path) = args.get("path").and_then(Value::as_str) {
                if raw_path.ends_with(".rs") {
                    if let Ok(p) = resolve_safe_path(workspace, raw_path, allow_outside) {
                        if let Ok(ast) = crate::code_ast::analyze_file(&p) {
                            if ast.ok {
                                out.push_str(&format!("\n[AST CHECK: OK ({} symbols)]", ast.symbol_count));
                            } else {
                                out.push_str(&format!("\n[AST SYNTAX WARNING: {} error(s): {:?}]", ast.error_count, ast.errors));
                            }
                        }
                    }
                }
            }
            Ok(out)
        }
        "search_code" => tool_search_code(workspace, allow_outside, args),
        "list_dir" => tool_list_dir(workspace, allow_outside, args),
        "run_command" => tool_run_command(workspace, allow_outside, args),
        "ast_symbols" => {
            let raw_path = args
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing required string argument 'path'".to_string())?;
            let path = resolve_safe_path(workspace, raw_path, allow_outside)?;
            let rep = crate::code_ast::analyze_file(&path)?;
            Ok(serde_json::to_string_pretty(&rep).unwrap_or_default())
        }
        "rhai_eval" => {
            let script = args
                .get("script")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing required string argument 'script'".to_string())?;
            let max_ops = args.get("max_ops").and_then(Value::as_u64).unwrap_or(50_000);
            let rep = crate::script_sandbox::eval_sandboxed(script, max_ops);
            Ok(serde_json::to_string_pretty(&rep).unwrap_or_default())
        }
        "megadaemon_tool" => match args.get("tool_name").and_then(Value::as_str) {
            Some(tname) if !tname.trim().is_empty() => {
                let targs = args.get("arguments").cloned().unwrap_or_else(|| json!({}));
                match crate::mcp_ext::dispatch(tname, &targs) {
                    Some(v) => Ok(serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())),
                    None => Err(format!("unknown MegaDaemon MCP tool '{}'", tname)),
                }
            }
            _ => Err("missing required string 'tool_name'".to_string()),
        },
        other => Err(format!("unknown tool '{}'", other)),
    });

    match res {
        Ok(output) => ToolExecResult { ok: true, output },
        Err(err) => ToolExecResult {
            ok: false,
            output: format!("ERROR: {}", err),
        },
    }
}

// ============================================================================
// 3. LLM Transport (Native Tool-Calling + Fenced-JSON / SEERE Fallback)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantTurn {
    pub content: String,
    pub reasoning: String,
    pub tool_calls: Vec<ToolCall>,
    pub tokens_used: u64,
    pub model_id: String,
}

/// Abstraction over LLM turns so the multi-turn ReAct loop is 100% unit-testable.
pub trait CodingBackend {
    fn next_turn(
        &mut self,
        messages: &[ChatMessage],
        tools: &Value,
        max_tokens: u32,
    ) -> Result<AssistantTurn, String>;

    fn fallback_used(&self) -> bool {
        false
    }
}

/// Calls an OpenAI-compatible `/chat/completions` endpoint with full message history
/// and `tools` definitions. Supports both native `tool_calls` and fenced-JSON fallback.
pub fn call_provider_messages(
    provider: &ProviderEntry,
    messages: &[ChatMessage],
    tools: &Value,
    max_tokens: u32,
) -> Result<AssistantTurn, String> {
    let api_key = resolve_provider_key(provider)?;
    let wire_messages: Vec<Value> = messages.iter().map(ChatMessage::to_openai_json).collect();

    let mut payload = json!({
        "model": provider.model,
        "messages": wire_messages,
        // v16: `max_tokens` acotado (igual que en `providers`), para que un
        // 400 del proveedor no se confunda con un fallo del proveedor.
        "max_tokens": max_tokens.min(crate::providers::MAX_TOKENS_CAP),
        "temperature": 0.2,
    });
    if let Some(arr) = tools.as_array() {
        if !arr.is_empty() {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("tools".to_string(), tools.clone());
                obj.insert("tool_choice".to_string(), Value::String("auto".to_string()));
            }
        }
    }

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .timeout_connect(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let mut req = agent.post(&provider.endpoint);
    match provider.auth_type.as_str() {
        "anthropic" => {
            if !api_key.is_empty() {
                req = req.header("x-api-key", &api_key);
            }
            req = req.header("anthropic-version", "2023-06-01");
        }
        "none" => {}
        _ => {
            if !api_key.is_empty() {
                req = req.header("Authorization", &format!("Bearer {}", api_key));
            }
        }
    }
    req = req
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://github.com/boolforge/megadaemon")
        .header("X-Title", "MegaDaemon Coding Engine");

    let body_bytes = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let resp = req
        .send(body_bytes.as_slice())
        // v16 (DEFECTO-02): la clave del proveedor NUNCA viaja en un error
        // (el cuerpo de error de un proveedor puede ecoarla).
        .map_err(|e| {
            crate::providers::redact_secret(
                &format!("HTTP transport error ({}): {}", provider.id, e),
                &api_key,
            )
        })?;

    let status = resp.status().as_u16();
    let text = resp
        .into_body()
        .read_to_string()
        .map_err(|e| {
            crate::providers::redact_secret(
                &format!("HTTP body read error ({}): {}", provider.id, e),
                &api_key,
            )
        })?;

    if status >= 400 {
        let detail = serde_json::from_str::<Value>(text.trim())
            .ok()
            .and_then(|v| {
                v.get("error")
                    .and_then(|e| {
                        e.get("message")
                            .and_then(Value::as_str)
                            .or_else(|| e.as_str())
                            .map(String::from)
                    })
                    .or_else(|| v.get("message").and_then(Value::as_str).map(String::from))
            })
            .unwrap_or_else(|| truncate_utf8_prefix(text.trim(), 200).to_string());
        return Err(crate::providers::redact_secret(
            &format!("HTTP {} ({}): {}", status, provider.id, detail),
            &api_key,
        ));
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(format!("empty HTTP response from {}", provider.id));
    }

    let v: Value = match serde_json::from_str(trimmed) {
        Ok(val) => val,
        Err(_) => {
            // Plain-text zero-key endpoint fallback.
            let (clean, calls) = parse_fenced_tool_calls(trimmed);
            return Ok(AssistantTurn {
                content: clean,
                reasoning: String::new(),
                tool_calls: calls,
                tokens_used: 0,
                model_id: provider.id.clone(),
            });
        }
    };

    if let Some(err_obj) = v.get("error") {
        let msg = err_obj
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| err_obj.as_str())
            .unwrap_or("API error");
        return Err(crate::providers::redact_secret(
            &format!("Provider API error ({}): {}", provider.id, msg),
            &api_key,
        ));
    }

    let msg_node = &v["choices"][0]["message"];
    let raw_content = msg_node["content"].as_str().unwrap_or("");
    let extra_reason = msg_node["reasoning_content"]
        .as_str()
        .or_else(|| msg_node["reasoning"].as_str())
        .unwrap_or("");
    let tokens = v["usage"]["total_tokens"].as_u64().unwrap_or(0);

    let (clean_content, think_tag) = split_think_tags(raw_content);
    let reasoning = if !extra_reason.is_empty() {
        extra_reason.to_string()
    } else {
        think_tag
    };

    // 1) Parse native OpenAI `tool_calls` array if present.
    let mut tool_calls = parse_native_tool_calls(msg_node);

    // 2) Fallback: parse fenced JSON `{"tool": "...", "args": {...}}` blocks if no native calls.
    let final_content = if tool_calls.is_empty() {
        let (stripped, parsed) = parse_fenced_tool_calls(&clean_content);
        tool_calls = parsed;
        stripped
    } else {
        clean_content
    };

    if final_content.is_empty() && tool_calls.is_empty() && reasoning.is_empty() {
        return Err(format!(
            "unexpected empty assistant turn from {}: {}",
            provider.id,
            truncate_utf8_prefix(trimmed, 200)
        ));
    }

    Ok(AssistantTurn {
        content: final_content,
        reasoning,
        tool_calls,
        tokens_used: tokens,
        model_id: provider.id.clone(),
    })
}

fn resolve_provider_key(provider: &ProviderEntry) -> Result<String, String> {
    match provider.auth_type.as_str() {
        "none" => Ok(String::new()),
        "gh_cli" => Command::new("gh")
            .args(["auth", "token"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("gh auth token unavailable for {}", provider.id)),
        "local" | "hybrid" => Err(format!(
            "provider '{}' is '{}' and cannot be called via OpenAI HTTP directly",
            provider.id, provider.auth_type
        )),
        _ => {
            let k = provider.api_key.trim();
            if k.is_empty() {
                Err(format!("missing API key for provider '{}'", provider.id))
            } else {
                Ok(k.to_string())
            }
        }
    }
}

fn parse_native_tool_calls(msg_node: &Value) -> Vec<ToolCall> {
    let mut out = Vec::new();
    let Some(arr) = msg_node.get("tool_calls").and_then(Value::as_array) else {
        return out;
    };
    for (idx, item) in arr.iter().enumerate() {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .unwrap_or_else(|| format!("call_native_{}", idx + 1));

        let func = item.get("function").unwrap_or(item);
        let Some(name) = func.get("name").and_then(Value::as_str) else {
            continue;
        };
        let raw_args = func.get("arguments");
        let arguments = match raw_args {
            Some(Value::String(s)) => serde_json::from_str::<Value>(s).unwrap_or_else(|_| json!({})),
            Some(Value::Object(_)) => raw_args.cloned().unwrap_or_else(|| json!({})),
            _ => json!({}),
        };
        out.push(ToolCall {
            id,
            name: name.to_string(),
            arguments,
        });
    }
    out
}

/// Extracts fallback tool calls of the form `{"tool": "<name>", "args": {...}}`
/// from fenced markdown code blocks (or a standalone JSON object) for models
/// and local SEERE that do not emit native OpenAI `tool_calls`.
pub fn parse_fenced_tool_calls(text: &str) -> (String, Vec<ToolCall>) {
    let mut calls = Vec::new();
    let mut remaining = String::new();
    let mut cursor = 0usize;

    while let Some(rel_start) = text[cursor..].find("```") {
        let fence_start = cursor + rel_start;
        let after_open = fence_start + 3;
        let Some(nl_rel) = text[after_open..].find('\n') else {
            break;
        };
        let body_start = after_open + nl_rel + 1;
        let Some(close_rel) = text[body_start..].find("```") else {
            break;
        };
        let body_end = body_start + close_rel;
        let block_body = text[body_start..body_end].trim();

        if let Some(mut extracted) = try_parse_tool_json(block_body, calls.len()) {
            remaining.push_str(&text[cursor..fence_start]);
            calls.append(&mut extracted);
        } else {
            remaining.push_str(&text[cursor..body_end + 3]);
        }
        cursor = body_end + 3;
    }

    remaining.push_str(&text[cursor..]);

    // If no fenced blocks yielded tool calls, check if the entire trimmed text is a tool JSON object.
    if calls.is_empty() {
        let trimmed = text.trim();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            if let Some(extracted) = try_parse_tool_json(trimmed, 0) {
                return (String::new(), extracted);
            }
        }
    }

    (remaining.trim().to_string(), calls)
}

fn try_parse_tool_json(raw: &str, id_offset: usize) -> Option<Vec<ToolCall>> {
    let val: Value = serde_json::from_str(raw).ok()?;
    let items: Vec<&Value> = match &val {
        Value::Array(arr) => arr.iter().collect(),
        Value::Object(_) => vec![&val],
        _ => return None,
    };
    let mut parsed = Vec::new();
    for (idx, item) in items.into_iter().enumerate() {
        let name = item
            .get("tool")
            .and_then(Value::as_str)
            .or_else(|| item.get("name").and_then(Value::as_str))?;
        let args = item
            .get("args")
            .or_else(|| item.get("arguments"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        parsed.push(ToolCall {
            id: format!("call_fb_{}", id_offset + idx + 1),
            name: name.to_string(),
            arguments: args,
        });
    }
    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
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

/// Live backend backed by MegaDaemon's provider vault with automatic failover
/// across configured cloud providers and guaranteed offline fallback to Local SEERE v12.
pub struct LiveVaultBackend {
    requested_model: Option<String>,
    fallback_used: bool,
}

impl LiveVaultBackend {
    pub fn new(requested_model: Option<String>) -> Self {
        Self {
            requested_model,
            fallback_used: false,
        }
    }

    fn run_local_seere_turn(&self, messages: &[ChatMessage]) -> AssistantTurn {
        let mut prompt = String::from(
            "You are MegaDaemon's offline coding agent. If you need a tool, reply with a fenced JSON block:\n\
             ```json\n{\"tool\": \"view_file|replace_file_content|write_to_file|search_code|list_dir|run_command\", \"args\": {...}}\n```\n\
             Otherwise give your concise final answer.\n\n",
        );
        for m in messages.iter().rev().take(6).rev() {
            let tag = match m.role {
                Role::System => "SYSTEM",
                Role::User => "USER",
                Role::Assistant => "ASSISTANT",
                Role::Tool => "TOOL",
            };
            prompt.push_str(&format!("[{}]\n{}\n\n", tag, truncate_utf8_prefix(&m.content, 1200)));
        }

        let (_, pol) = crate::power_policy::current_policy();
        let res = crate::seere_v11::run_seere_v11(
            &prompt,
            pol.max_turns.max(1),
            true,
            false,
            false,
            true,
            true,
        );
        let ans = res["answer"]
            .as_str()
            .unwrap_or("Local SEERE produced no output.")
            .to_string();
        let tokens = res["tokens"].as_u64().unwrap_or(0);
        let (clean, calls) = parse_fenced_tool_calls(&ans);

        AssistantTurn {
            content: clean,
            reasoning: String::new(),
            tool_calls: calls,
            tokens_used: tokens,
            model_id: "local-seere".to_string(),
        }
    }
}

impl CodingBackend for LiveVaultBackend {
    fn next_turn(
        &mut self,
        messages: &[ChatMessage],
        tools: &Value,
        max_tokens: u32,
    ) -> Result<AssistantTurn, String> {
        let vault = providers::load_vault();
        let target_id = self
            .requested_model
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&vault.default_model);

        if let Some((rid, rmodel, rnote)) = crate::provider_catalog::is_retired(target_id) {
            return Err(format!(
                "retired model requested: {} ({}) -> {}",
                rmodel, rid, rnote
            ));
        }

        if target_id == "local-seere" || target_id == "local" {
            return Ok(self.run_local_seere_turn(messages));
        }

        let mut candidates: Vec<ProviderEntry> = Vec::new();
        if let Some(p) = vault.providers.iter().find(|p| p.id == target_id) {
            if p.auth_type != "local" && p.auth_type != "hybrid" {
                candidates.push(p.clone());
            }
        }
        for p in &vault.providers {
            if p.id != target_id
                && p.enabled
                && p.auth_type != "local"
                && p.auth_type != "hybrid"
            {
                candidates.push(p.clone());
            }
        }

        let mut errors: Vec<String> = Vec::new();
        for (idx, p) in candidates.iter().enumerate() {
            if resolve_provider_key(p).is_err() {
                continue;
            }
            match call_provider_messages(p, messages, tools, max_tokens) {
                Ok(turn) => {
                    if idx > 0 {
                        self.fallback_used = true;
                    }
                    return Ok(turn);
                }
                Err(e) => errors.push(e),
            }
        }

        // Guaranteed offline fallback to Local SEERE v12.
        self.fallback_used = true;
        let mut turn = self.run_local_seere_turn(messages);
        if !errors.is_empty() && turn.tool_calls.is_empty() {
            turn.content = format!(
                "{}\n\n[Offline SEERE fallback active; cloud errors: {}]",
                turn.content,
                errors.join(" | ")
            );
        }
        Ok(turn)
    }

    fn fallback_used(&self) -> bool {
        self.fallback_used
    }
}

// ============================================================================
// 4. Multi-Turn Coding Agent Loop
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingAgentConfig {
    pub workspace_root: PathBuf,
    pub max_turns: usize,
    pub max_context_chars: usize,
    pub model: Option<String>,
    pub session_id: Option<String>,
    pub allow_outside_workspace: bool,
}

impl Default for CodingAgentConfig {
    fn default() -> Self {
        Self {
            workspace_root: PathBuf::from("."),
            max_turns: 20,
            max_context_chars: 64_000,
            model: None,
            session_id: None,
            allow_outside_workspace: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnTrace {
    pub turn: usize,
    pub model_id: String,
    pub tools_called: Vec<String>,
    pub tool_errors: usize,
    pub tokens_used: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingSessionReport {
    pub session_id: String,
    pub session_path: String,
    pub completed: bool,
    pub final_answer: String,
    pub turns_taken: usize,
    pub total_tokens: u64,
    pub tool_calls_executed: usize,
    pub files_modified: Vec<String>,
    pub model_id: String,
    pub fallback_used: bool,
    pub compactions: usize,
    pub loop_interventions: usize,
    pub error: Option<String>,
    pub trace: Vec<TurnTrace>,
}

fn coding_system_prompt(workspace: &Path) -> String {
    format!(
        "You are MegaDaemon's autonomous software engineering agent operating in workspace '{}'.\n\
         Use the provided tools (`view_file`, `replace_file_content`, `write_to_file`, `search_code`, `list_dir`, `run_command`) \
         to inspect code, make minimal surgical edits, and verify your changes.\n\
         Rules:\n\
         1. Always read relevant lines with `view_file` before modifying a file.\n\
         2. Prefer `replace_file_content` with exact context over rewriting whole files.\n\
         3. Never repeat an identical failing tool call; inspect the error and adapt.\n\
         4. Once the goal is verified, respond with a concise summary without further tool calls.",
        workspace.display()
    )
}

/// Runs a multi-turn coding session using MegaDaemon's live provider vault + SEERE fallback.
pub fn run_coding_session_live(goal: &str, cfg: &CodingAgentConfig) -> CodingSessionReport {
    let mut backend = LiveVaultBackend::new(cfg.model.clone());
    run_coding_session(goal, cfg, &mut backend)
}

/// Runs a multi-turn coding session with an injected `CodingBackend` (persisting to default `state_dir`).
pub fn run_coding_session(
    goal: &str,
    cfg: &CodingAgentConfig,
    backend: &mut dyn CodingBackend,
) -> CodingSessionReport {
    let sid = cfg
        .session_id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("code_{}", now_unix_secs()));
    let mut store = SessionStore::new(sid);
    run_coding_session_with_store(goal, cfg, backend, &mut store)
}

/// Core multi-turn ReAct loop operating on an explicit `SessionStore` (ideal for tests & resuming).
pub fn run_coding_session_with_store(
    goal: &str,
    cfg: &CodingAgentConfig,
    backend: &mut dyn CodingBackend,
    store: &mut SessionStore,
) -> CodingSessionReport {
    // Resume existing session if present on disk; otherwise seed system prompt.
    if store.messages().is_empty() && store.path().exists() {
        let _ = store.reload();
    }
    if store.messages().is_empty() {
        let _ = store.push_and_persist(ChatMessage::system(coding_system_prompt(
            &cfg.workspace_root,
        )));
    }
    let _ = store.push_and_persist(ChatMessage::user(goal));

    let tools = tool_definitions();
    let max_turns = cfg.max_turns.max(1);

    let mut total_tokens: u64 = 0;
    let mut tool_calls_executed: usize = 0;
    let mut modified_set: HashSet<String> = HashSet::new();
    let mut trace: Vec<TurnTrace> = Vec::new();
    let mut failing_signatures: HashMap<String, usize> = HashMap::new();
    let mut compactions: usize = 0;
    let mut loop_interventions: usize = 0;
    let mut last_model_id = String::new();
    let mut final_answer = String::new();
    let mut completed = false;
    let mut error: Option<String> = None;

    for turn_idx in 1..=max_turns {
        if store.compact_if_needed(cfg.max_context_chars) {
            compactions += 1;
            let _ = store.save_all();
        }

        let turn = match backend.next_turn(store.messages(), &tools, DEFAULT_MAX_TOKENS) {
            Ok(t) => t,
            Err(e) => {
                error = Some(e);
                break;
            }
        };

        total_tokens = total_tokens.saturating_add(turn.tokens_used);
        if !turn.model_id.is_empty() {
            last_model_id = turn.model_id.clone();
        }

        if turn.tool_calls.is_empty() {
            final_answer = turn.content.clone();
            let _ = store.push_and_persist(ChatMessage::assistant(turn.content));
            trace.push(TurnTrace {
                turn: turn_idx,
                model_id: turn.model_id,
                tools_called: Vec::new(),
                tool_errors: 0,
                tokens_used: turn.tokens_used,
            });
            completed = true;
            break;
        }

        // Persist assistant turn with its requested tool calls.
        let _ = store.push_and_persist(ChatMessage::assistant_with_tools(
            turn.content.clone(),
            turn.tool_calls.clone(),
        ));

        let mut called_names = Vec::new();
        let mut turn_errors = 0usize;
        let mut loop_hint: Option<String> = None;
        let mut abort_loop = false;

        for tc in &turn.tool_calls {
            called_names.push(tc.name.clone());
            tool_calls_executed += 1;

            let exec = dispatch_tool(
                &tc.name,
                &tc.arguments,
                &cfg.workspace_root,
                cfg.allow_outside_workspace,
            );

            if exec.ok {
                if tc.name == "replace_file_content" || tc.name == "write_to_file" {
                    if let Some(p) = tc.arguments.get("path").and_then(Value::as_str) {
                        modified_set.insert(p.to_string());
                    }
                }
            } else {
                turn_errors += 1;
                let sig = format!("{}::{}", tc.name, tc.arguments);
                let count = failing_signatures.entry(sig).or_insert(0);
                *count += 1;
                if *count >= 4 {
                    abort_loop = true;
                    error = Some(format!(
                        "aborted after {} identical failing calls to '{}'",
                        *count, tc.name
                    ));
                } else if *count >= 2 {
                    loop_interventions += 1;
                    loop_hint = Some(format!(
                        "LOOP GUARD: Your tool call '{}' with identical arguments has failed {} times. \
                         Do NOT repeat the same call. Inspect the file or error output and try a different approach.",
                        tc.name, *count
                    ));
                }
            }

            let _ = store.push_and_persist(ChatMessage::tool_result(
                tc.id.clone(),
                tc.name.clone(),
                exec.output,
            ));
        }

        trace.push(TurnTrace {
            turn: turn_idx,
            model_id: turn.model_id,
            tools_called: called_names,
            tool_errors: turn_errors,
            tokens_used: turn.tokens_used,
        });

        if abort_loop {
            break;
        }

        if let Some(hint) = loop_hint {
            let _ = store.push_and_persist(ChatMessage::system(hint));
        }
    }

    if !completed && error.is_none() {
        error = Some(format!("reached max_turns ({}) without final answer", max_turns));
    }

    let mut files_modified: Vec<String> = modified_set.into_iter().collect();
    files_modified.sort();

    CodingSessionReport {
        session_id: store.session_id().to_string(),
        session_path: store.path().display().to_string(),
        completed,
        final_answer,
        turns_taken: trace.len(),
        total_tokens,
        tool_calls_executed,
        files_modified,
        model_id: last_model_id,
        fallback_used: backend.fallback_used(),
        compactions,
        loop_interventions,
        error,
        trace,
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn truncate_utf8_prefix(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn truncate_utf8_suffix(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut start = s.len() - max_bytes;
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

// ============================================================================
// 5. Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempWorkspace {
        dir: PathBuf,
    }

    impl TempWorkspace {
        fn new(label: &str) -> Self {
            let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "md_agent_coding_{}_{}_{}",
                label,
                std::process::id(),
                seq
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("create temp workspace");
            Self { dir }
        }

        fn path(&self) -> &Path {
            &self.dir
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    struct MockBackend {
        turns: VecDeque<Result<AssistantTurn, String>>,
        seen_message_counts: Vec<usize>,
    }

    impl MockBackend {
        fn new(turns: Vec<AssistantTurn>) -> Self {
            Self {
                turns: turns.into_iter().map(Ok).collect(),
                seen_message_counts: Vec::new(),
            }
        }
    }

    impl CodingBackend for MockBackend {
        fn next_turn(
            &mut self,
            messages: &[ChatMessage],
            _tools: &Value,
            _max_tokens: u32,
        ) -> Result<AssistantTurn, String> {
            self.seen_message_counts.push(messages.len());
            self.turns
                .pop_front()
                .unwrap_or_else(|| Err("mock backend exhausted".to_string()))
        }
    }

    #[test]
    fn test_view_file_window_and_bounds() {
        let ws = TempWorkspace::new("view");
        let content = (1..=10)
            .map(|i| format!("line number {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(ws.path().join("sample.txt"), content).unwrap();

        let res = tool_view_file(
            ws.path(),
            false,
            &json!({"path": "sample.txt", "start_line": 3, "end_line": 5}),
        )
        .unwrap();
        assert!(res.contains("showing lines 3-5 of 10"));
        assert!(res.contains("3: line number 3"));
        assert!(res.contains("5: line number 5"));
        assert!(!res.contains("6: line number 6"));

        let err = tool_view_file(
            ws.path(),
            false,
            &json!({"path": "sample.txt", "start_line": 99}),
        )
        .unwrap_err();
        assert!(err.contains("exceeds total line count"));
    }

    #[test]
    fn test_replace_file_content_exact_ambiguity_and_atomic() {
        let ws = TempWorkspace::new("replace");
        let fp = ws.path().join("lib.rs");
        fs::write(&fp, "fn foo() { let x = 1; }\nfn bar() { let x = 1; }\n").unwrap();

        // Ambiguous without allow_multiple
        let err = tool_replace_file_content(
            ws.path(),
            false,
            &json!({
                "path": "lib.rs",
                "target_content": "let x = 1;",
                "replacement_content": "let x = 2;"
            }),
        )
        .unwrap_err();
        assert!(err.contains("matched 2 times"));

        // Exact unique match succeeds and returns diff
        let ok = tool_replace_file_content(
            ws.path(),
            false,
            &json!({
                "path": "lib.rs",
                "target_content": "fn foo() { let x = 1; }",
                "replacement_content": "fn foo() { let x = 42; }"
            }),
        )
        .unwrap();
        assert!(ok.contains("--- a/lib.rs"));
        assert!(ok.contains("+fn foo() { let x = 42; }"));

        let after = fs::read_to_string(&fp).unwrap();
        assert!(after.contains("let x = 42;"));
        assert!(after.contains("fn bar() { let x = 1; }"));

        // Missing target fails safely
        let miss = tool_replace_file_content(
            ws.path(),
            false,
            &json!({
                "path": "lib.rs",
                "target_content": "nonexistent_symbol",
                "replacement_content": "x"
            }),
        )
        .unwrap_err();
        assert!(miss.contains("0 matches"));
    }

    #[test]
    fn test_write_to_file_create_and_overwrite_guard() {
        let ws = TempWorkspace::new("write");
        let ok = tool_write_to_file(
            ws.path(),
            false,
            &json!({"path": "nested/dir/hello.txt", "content": "hello\nworld\n"}),
        )
        .unwrap();
        assert!(ok.contains("2 lines"));
        assert_eq!(
            fs::read_to_string(ws.path().join("nested/dir/hello.txt")).unwrap(),
            "hello\nworld\n"
        );

        // Overwrite false rejects existing file
        let err = tool_write_to_file(
            ws.path(),
            false,
            &json!({"path": "nested/dir/hello.txt", "content": "new"}),
        )
        .unwrap_err();
        assert!(err.contains("already exists"));

        // Overwrite true succeeds
        tool_write_to_file(
            ws.path(),
            false,
            &json!({"path": "nested/dir/hello.txt", "content": "updated", "overwrite": true}),
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(ws.path().join("nested/dir/hello.txt")).unwrap(),
            "updated"
        );
    }

    #[test]
    fn test_search_code_skips_ignored_dirs() {
        let ws = TempWorkspace::new("search");
        fs::create_dir_all(ws.path().join("src")).unwrap();
        fs::create_dir_all(ws.path().join(".git")).unwrap();
        fs::create_dir_all(ws.path().join("target")).unwrap();
        fs::create_dir_all(ws.path().join("node_modules")).unwrap();

        fs::write(ws.path().join("src/main.rs"), "fn needle_fn() {}\n").unwrap();
        fs::write(ws.path().join(".git/config"), "needle_fn\n").unwrap();
        fs::write(ws.path().join("target/out.txt"), "needle_fn\n").unwrap();
        fs::write(ws.path().join("node_modules/pkg.js"), "needle_fn\n").unwrap();

        let res = tool_search_code(ws.path(), false, &json!({"query": "needle_fn"})).unwrap();
        assert!(res.contains("src/main.rs:1: fn needle_fn() {}"));
        assert!(!res.contains(".git"));
        assert!(!res.contains("target"));
        assert!(!res.contains("node_modules"));
    }

    #[test]
    fn test_list_dir_and_path_sandbox() {
        let ws = TempWorkspace::new("listdir");
        fs::create_dir_all(ws.path().join("alpha_dir")).unwrap();
        fs::write(ws.path().join("beta.txt"), "12345").unwrap();

        let out = tool_list_dir(ws.path(), false, &json!({})).unwrap();
        assert!(out.contains("dir  alpha_dir/"));
        assert!(out.contains("file beta.txt (5 B)"));

        // Sandbox rejects parent traversal
        let esc = resolve_safe_path(ws.path(), "../../etc/passwd", false).unwrap_err();
        assert!(esc.contains("outside workspace root"));
    }

    #[test]
    fn test_run_command_output_and_timeout() {
        let ws = TempWorkspace::new("cmd");
        let ok = tool_run_command(
            ws.path(),
            false,
            &json!({"command": "echo hello_megadaemon", "timeout_secs": 5}),
        )
        .unwrap();
        assert!(ok.contains("exit_code: 0"));
        assert!(ok.contains("hello_megadaemon"));

        let to = tool_run_command(
            ws.path(),
            false,
            &json!({"command": "sleep 5", "timeout_secs": 1}),
        )
        .unwrap();
        assert!(to.contains("timed_out: true"));
    }

    #[test]
    fn test_session_store_jsonl_roundtrip_and_compaction() {
        let ws = TempWorkspace::new("session");
        let mut store = SessionStore::with_base_dir(ws.path(), "sess-01");
        store.push_and_persist(ChatMessage::system("SYSTEM PROMPT")).unwrap();
        store.push_and_persist(ChatMessage::user("Fix the bug")).unwrap();
        for i in 0..8 {
            store
                .push_and_persist(ChatMessage::assistant_with_tools(
                    format!("step {}", i),
                    vec![ToolCall {
                        id: format!("c{}", i),
                        name: "view_file".to_string(),
                        arguments: json!({"path": "a.rs"}),
                    }],
                ))
                .unwrap();
            store
                .push_and_persist(ChatMessage::tool_result(
                    format!("c{}", i),
                    "view_file",
                    "X".repeat(1200),
                ))
                .unwrap();
        }

        // Reload from disk and verify roundtrip
        let mut reloaded = SessionStore::with_base_dir(ws.path(), "sess-01");
        let count = reloaded.reload().unwrap();
        assert_eq!(count, store.messages().len());
        assert_eq!(reloaded.messages()[0].content, "SYSTEM PROMPT");
        assert_eq!(reloaded.messages()[1].content, "Fix the bug");

        // Compact aggressively and verify system + first user + recent tail preserved
        let before_chars = reloaded.total_chars();
        let did_compact = reloaded.compact_if_needed(2500);
        assert!(did_compact);
        assert!(reloaded.total_chars() < before_chars);
        assert_eq!(reloaded.messages()[0].role, Role::System);
        assert_eq!(reloaded.messages()[0].content, "SYSTEM PROMPT");
        assert_eq!(reloaded.messages()[1].role, Role::User);
        assert_eq!(reloaded.messages()[1].content, "Fix the bug");
    }

    #[test]
    fn test_fenced_tool_call_fallback_parser() {
        let raw = "Let me check the file.\n```json\n{\"tool\": \"view_file\", \"args\": {\"path\": \"src/lib.rs\"}}\n```\n";
        let (clean, calls) = parse_fenced_tool_calls(raw);
        assert_eq!(clean, "Let me check the file.");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "view_file");
        assert_eq!(calls[0].arguments["path"], "src/lib.rs");
    }

    #[test]
    fn test_multi_turn_loop_read_edit_verify_finish() {
        let ws = TempWorkspace::new("e2e");
        let sessions_dir = ws.path().join(".sessions");
        fs::write(ws.path().join("calc.txt"), "value = 10\n").unwrap();

        let mut backend = MockBackend::new(vec![
            // Turn 1: read file
            AssistantTurn {
                content: "Inspecting calc.txt".to_string(),
                reasoning: String::new(),
                tool_calls: vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "view_file".to_string(),
                    arguments: json!({"path": "calc.txt"}),
                }],
                tokens_used: 100,
                model_id: "mock-coder".to_string(),
            },
            // Turn 2: edit file
            AssistantTurn {
                content: "Updating value to 42".to_string(),
                reasoning: String::new(),
                tool_calls: vec![ToolCall {
                    id: "call_2".to_string(),
                    name: "replace_file_content".to_string(),
                    arguments: json!({
                        "path": "calc.txt",
                        "target_content": "value = 10",
                        "replacement_content": "value = 42"
                    }),
                }],
                tokens_used: 120,
                model_id: "mock-coder".to_string(),
            },
            // Turn 3: verify with shell command
            AssistantTurn {
                content: "Verifying".to_string(),
                reasoning: String::new(),
                tool_calls: vec![ToolCall {
                    id: "call_3".to_string(),
                    name: "run_command".to_string(),
                    arguments: json!({"command": "cat calc.txt"}),
                }],
                tokens_used: 80,
                model_id: "mock-coder".to_string(),
            },
            // Turn 4: final answer
            AssistantTurn {
                content: "Updated calc.txt to value = 42 and verified.".to_string(),
                reasoning: String::new(),
                tool_calls: Vec::new(),
                tokens_used: 60,
                model_id: "mock-coder".to_string(),
            },
        ]);

        let cfg = CodingAgentConfig {
            workspace_root: ws.path().to_path_buf(),
            max_turns: 10,
            max_context_chars: 32_000,
            model: Some("mock-coder".to_string()),
            session_id: Some("test_e2e".to_string()),
            allow_outside_workspace: false,
        };
        let mut store = SessionStore::with_base_dir(&sessions_dir, "test_e2e");
        let report = run_coding_session_with_store(
            "Change value to 42 in calc.txt",
            &cfg,
            &mut backend,
            &mut store,
        );

        assert!(report.completed, "report error: {:?}", report.error);
        assert_eq!(report.turns_taken, 4);
        assert_eq!(report.tool_calls_executed, 3);
        assert_eq!(report.total_tokens, 360);
        assert_eq!(report.files_modified, vec!["calc.txt".to_string()]);
        assert_eq!(
            fs::read_to_string(ws.path().join("calc.txt")).unwrap(),
            "value = 42\n"
        );
        assert!(store.path().exists());
    }

    #[test]
    fn test_loop_detection_injects_corrective_hint() {
        let ws = TempWorkspace::new("loop_guard");
        let sessions_dir = ws.path().join(".sessions");
        fs::write(ws.path().join("a.txt"), "hello\n").unwrap();

        let bad_call = ToolCall {
            id: "bad".to_string(),
            name: "replace_file_content".to_string(),
            arguments: json!({
                "path": "a.txt",
                "target_content": "NOT_THERE",
                "replacement_content": "X"
            }),
        };

        let mut backend = MockBackend::new(vec![
            AssistantTurn {
                content: String::new(),
                reasoning: String::new(),
                tool_calls: vec![bad_call.clone()],
                tokens_used: 10,
                model_id: "mock".to_string(),
            },
            AssistantTurn {
                content: String::new(),
                reasoning: String::new(),
                tool_calls: vec![bad_call],
                tokens_used: 10,
                model_id: "mock".to_string(),
            },
            AssistantTurn {
                content: "Stopped after loop hint.".to_string(),
                reasoning: String::new(),
                tool_calls: Vec::new(),
                tokens_used: 10,
                model_id: "mock".to_string(),
            },
        ]);

        let cfg = CodingAgentConfig {
            workspace_root: ws.path().to_path_buf(),
            max_turns: 6,
            max_context_chars: 16_000,
            model: None,
            session_id: Some("loop_test".to_string()),
            allow_outside_workspace: false,
        };
        let mut store = SessionStore::with_base_dir(&sessions_dir, "loop_test");
        let report = run_coding_session_with_store("Try edit", &cfg, &mut backend, &mut store);

        assert!(report.completed);
        assert_eq!(report.loop_interventions, 1);
        assert!(store
            .messages()
            .iter()
            .any(|m| m.role == Role::System && m.content.contains("LOOP GUARD")));
    }

    #[test]
    fn test_megadaemon_tool_bridges_mcp_ext() {
        let ws = TempWorkspace::new("mcp_bridge");
        let res = dispatch_tool(
            "megadaemon_tool",
            &json!({
                "tool_name": "es_eid",
                "arguments": {
                    "action": "dnie_atr",
                    "atr_hex": "3B00"
                }
            }),
            ws.path(),
            false,
        );
        assert!(res.ok, "megadaemon_tool failed: {}", res.output);
        let outer: Value = serde_json::from_str(&res.output).expect("valid outer json");
        let inner_txt = outer["content"][0]["text"].as_str().expect("text payload");
        let inner: Value = serde_json::from_str(inner_txt).expect("valid inner json");
        assert_eq!(inner["is_dnie_3_4"], false);

        let missing = dispatch_tool(
            "megadaemon_tool",
            &json!({ "tool_name": "nonexistent_tool_xyz" }),
            ws.path(),
            false,
        );
        assert!(!missing.ok);
    }

    #[test]
    fn test_ast_symbols_rhai_and_post_edit_check() {
        let ws = TempWorkspace::new("ast_rhai");
        let wr = dispatch_tool(
            "write_to_file",
            &json!({
                "path": "sample.rs",
                "content": "pub fn greet() -> i32 { 7 }\n",
                "overwrite": true
            }),
            ws.path(),
            false,
        );
        assert!(wr.ok, "write_to_file failed: {}", wr.output);
        assert!(wr.output.contains("[AST CHECK: OK"));

        let ast = dispatch_tool(
            "ast_symbols",
            &json!({ "path": "sample.rs" }),
            ws.path(),
            false,
        );
        assert!(ast.ok);
        assert!(ast.output.contains("greet"));

        let rhai = dispatch_tool(
            "rhai_eval",
            &json!({ "script": "6 * 7" }),
            ws.path(),
            false,
        );
        assert!(rhai.ok);
        assert!(rhai.output.contains("\"value\": \"42\""));
    }
}
