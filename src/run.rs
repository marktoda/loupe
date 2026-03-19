use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Run {
    pub id: usize,
    pub path: PathBuf,
    pub status: RunStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub items: Vec<TranscriptItem>,
    pub stats: RunStats,
    pub bytes_read: u64,
    pub result: Option<SessionResult>,
    pub last_modified: Option<std::time::SystemTime>,
    pub tool_timestamps: std::collections::HashMap<usize, Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
    Unknown,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TranscriptItem {
    SessionStart {
        model: String,
        tools: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    AssistantText {
        text: String,
        is_partial: bool,
    },
    ToolUse {
        name: String,
        summary: String,
        input: Option<Value>,
    },
    ToolResult {
        tool_name: String,
        summary: String,
        content: Option<String>,
        duration_ms: Option<u64>,
    },
    SubagentStart {
        description: String,
        task_id: String,
    },
    SubagentProgress {
        description: String,
        tool_name: Option<String>,
    },
    SubagentEnd {
        summary: String,
        status: String,
        cost_usd: Option<f64>,
        duration_ms: Option<u64>,
        tool_uses: Option<u64>,
        total_tokens: Option<u64>,
    },
    Error {
        message: String,
    },
    SystemEvent {
        label: String,
        detail: String,
    },
    Thinking {
        text: String,
    },
    RunResult {
        is_error: bool,
        stop_reason: Option<String>,
        num_turns: u64,
        total_cost_usd: f64,
        duration_ms: u64,
        result_text: Option<String>,
    },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
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
    pub num_turns: u64,
    pub token_count: u64,
}

impl Run {
    pub fn new(id: usize, path: PathBuf) -> Self {
        Self {
            id,
            path,
            status: RunStatus::Unknown,
            started_at: None,
            ended_at: None,
            session_id: None,
            model: None,
            items: Vec::new(),
            stats: RunStats::default(),
            bytes_read: 0,
            result: None,
            last_modified: None,
            tool_timestamps: std::collections::HashMap::new(),
        }
    }

    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.started_at, self.ended_at) {
            (Some(start), Some(end)) => Some(end - start),
            (Some(start), None) => Some(Utc::now() - start),
            _ => None,
        }
    }
}

impl RunStats {
    pub fn merge(&mut self, other: &RunStats) {
        self.assistant_chars += other.assistant_chars;
        self.tool_calls += other.tool_calls;
        self.subagent_spawns += other.subagent_spawns;
        self.parse_errors += other.parse_errors;
        self.total_lines += other.total_lines;
        if other.cost_usd.is_some() {
            self.cost_usd = other.cost_usd;
        }
        if other.num_turns > 0 {
            self.num_turns = other.num_turns;
        }
        if other.token_count > 0 {
            self.token_count = other.token_count;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_run_has_unknown_status() {
        let run = Run::new(0, PathBuf::from("test.jsonl"));
        assert_eq!(run.status, RunStatus::Unknown);
        assert!(run.items.is_empty());
        assert_eq!(run.bytes_read, 0);
    }

    #[test]
    fn run_stats_merge() {
        let mut base = RunStats {
            tool_calls: 3,
            total_lines: 10,
            ..Default::default()
        };
        let delta = RunStats {
            tool_calls: 2,
            total_lines: 5,
            cost_usd: Some(1.5),
            ..Default::default()
        };
        base.merge(&delta);
        assert_eq!(base.tool_calls, 5);
        assert_eq!(base.total_lines, 15);
        assert_eq!(base.cost_usd, Some(1.5));
    }

    #[test]
    fn duration_returns_none_without_start() {
        let run = Run::new(0, PathBuf::from("test.jsonl"));
        assert!(run.duration().is_none());
    }

    #[test]
    fn run_stats_merge_new_fields() {
        let mut base = RunStats {
            num_turns: 5,
            token_count: 1000,
            ..Default::default()
        };
        let delta = RunStats {
            num_turns: 10,
            token_count: 5000,
            ..Default::default()
        };
        base.merge(&delta);
        assert_eq!(base.num_turns, 10); // num_turns is replaced, not summed
        assert_eq!(base.token_count, 5000); // token_count is replaced, not summed
    }
}
