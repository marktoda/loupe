use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::PathBuf;

// Stub types so events.rs compiles
#[derive(Debug, Clone)]
pub struct Run;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus { Running, Completed, Failed, Unknown }

#[derive(Debug, Clone)]
pub enum TranscriptItem {
    SessionStart { model: String, tools: Vec<String>, timestamp: DateTime<Utc> },
    AssistantText { text: String, is_partial: bool },
    ToolUse { name: String, summary: String, input: Option<Value> },
    ToolResult { tool_name: String, summary: String, content: Option<String> },
    SubagentStart { description: String, task_id: String },
    SubagentProgress { description: String, tool_name: Option<String> },
    SubagentEnd { summary: String, status: String, cost_usd: Option<f64> },
    Error { message: String },
    SystemEvent { label: String, detail: String },
}

#[derive(Debug, Clone)]
pub struct SessionResult {
    pub subtype: String,
    pub is_error: bool,
    pub duration_ms: u64,
    pub num_turns: u64,
    pub total_cost_usd: f64,
    pub stop_reason: Option<String>,
    pub result_text: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RunStats {
    pub assistant_chars: usize,
    pub tool_calls: usize,
    pub subagent_spawns: usize,
    pub parse_errors: usize,
    pub total_lines: usize,
    pub cost_usd: Option<f64>,
}
