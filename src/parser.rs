use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::run::{RunStats, SessionResult, TranscriptItem};

/// Detected transcript format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    ClaudeCode,
    Codex,
}

/// Trait for parsing JSONL transcript lines into TranscriptItems.
pub trait TranscriptParser: Send + Sync {
    fn parse_line(&self, line: &str) -> ParseResult;
}

/// Detect transcript format from the first JSONL line.
/// Codex wraps all events in `{ timestamp, type, payload }`.
/// Claude Code has flat top-level events with no `payload` wrapper.
pub fn detect_format(first_line: &str) -> Option<Format> {
    let v: serde_json::Value = serde_json::from_str(first_line).ok()?;
    if v.get("payload").is_some() {
        Some(Format::Codex)
    } else {
        Some(Format::ClaudeCode)
    }
}

/// Parser for Claude Code's JSONL stream format.
pub struct ClaudeCodeParser;

impl TranscriptParser for ClaudeCodeParser {
    fn parse_line(&self, line: &str) -> ParseResult {
        parse_line(line)
    }
}

/// Metadata extracted from a line that applies to the Run, not the transcript
#[derive(Debug, Default)]
pub struct LineMeta {
    pub session_id: Option<String>,
    pub session_result: Option<SessionResult>,
    pub stats_delta: RunStats,
    pub timestamp: Option<DateTime<Utc>>,
}

/// Result of parsing a single JSONL line.
pub enum ParseResult {
    /// Recognized event with transcript items and metadata
    Parsed(Vec<TranscriptItem>, LineMeta),
    /// Intentionally skipped (stream_event in Tier 1, user without tool_result)
    Skipped,
    /// Failed to parse — malformed JSON or missing required fields
    Error(String),
}

pub fn parse_line(line: &str) -> ParseResult {
    let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return ParseResult::Error(format!("JSON parse error: {e}")),
    };
    let Some(event_type) = v.get("type").and_then(|t| t.as_str()) else {
        return ParseResult::Error("Missing 'type' field".to_string());
    };
    let mut meta = LineMeta {
        session_id: v
            .get("session_id")
            .and_then(|s| s.as_str())
            .map(String::from),
        ..Default::default()
    };

    match event_type {
        "stream_event" => ParseResult::Skipped,
        "system" => {
            let items = parse_system(&v, &mut meta);
            ParseResult::Parsed(items, meta)
        }
        "assistant" => {
            let items = parse_assistant(&v, &mut meta);
            ParseResult::Parsed(items, meta)
        }
        "user" => {
            let items = parse_user(&v);
            if items.is_empty() {
                ParseResult::Skipped
            } else {
                ParseResult::Parsed(items, meta)
            }
        }
        "result" => {
            let items = parse_result_event(&v, &mut meta);
            ParseResult::Parsed(items, meta)
        }
        "rate_limit_event" => ParseResult::Parsed(
            vec![TranscriptItem::SystemEvent {
                label: "rate_limit".to_string(),
                detail: v
                    .get("rate_limit_info")
                    .and_then(|r| r.get("status"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
            }],
            meta,
        ),
        other => ParseResult::Parsed(
            vec![TranscriptItem::SystemEvent {
                label: other.to_string(),
                detail: serde_json::to_string(&v).unwrap_or_default(),
            }],
            meta,
        ),
    }
}

/// Parse an RFC3339 timestamp from a JSON object field.
pub fn parse_timestamp(v: &Value, field: &str) -> Option<DateTime<Utc>> {
    v.get(field)
        .and_then(|t| t.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

/// Truncate a string to at most `max` bytes, respecting UTF-8 char boundaries.
pub fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    // Find the last char boundary at or before max
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn parse_system(v: &Value, meta: &mut LineMeta) -> Vec<TranscriptItem> {
    let subtype = v.get("subtype").and_then(|s| s.as_str()).unwrap_or("");

    meta.timestamp = parse_timestamp(v, "timestamp");

    match subtype {
        "init" => {
            let model = v
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let tools: Vec<String> = v
                .get("tools")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| {
                            // Tools can be strings or objects with a "name" field
                            item.as_str().map(String::from).or_else(|| {
                                item.get("name").and_then(|n| n.as_str()).map(String::from)
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let timestamp = meta.timestamp.unwrap_or_else(Utc::now);
            vec![TranscriptItem::SessionStart {
                model,
                tools,
                timestamp,
            }]
        }
        "task_started" => {
            let description = v
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            let task_id = v
                .get("task_id")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            vec![TranscriptItem::SubagentStart {
                description,
                task_id,
            }]
        }
        "task_progress" => {
            let description = v
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            let tool_name = v
                .get("last_tool_name")
                .and_then(|t| t.as_str())
                .map(String::from);
            vec![TranscriptItem::SubagentProgress {
                description,
                tool_name,
            }]
        }
        "task_notification" => {
            let summary = v
                .get("summary")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let status = v
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let usage = v.get("usage");
            let duration_ms =
                usage.and_then(|u| u.get("duration_ms")).and_then(|d| d.as_u64());
            let tool_uses =
                usage.and_then(|u| u.get("tool_uses")).and_then(|t| t.as_u64());
            let total_tokens =
                usage.and_then(|u| u.get("total_tokens")).and_then(|t| t.as_u64());
            vec![TranscriptItem::SubagentEnd {
                summary,
                status,
                cost_usd: None,
                duration_ms,
                tool_uses,
                total_tokens,
            }]
        }
        other => {
            let detail = v
                .get("summary")
                .and_then(|s| s.as_str())
                .map(String::from)
                .unwrap_or_else(|| serde_json::to_string(v).unwrap_or_default());
            let detail = if detail.is_empty() {
                serde_json::to_string(v).unwrap_or_default()
            } else {
                detail
            };
            vec![TranscriptItem::SystemEvent {
                label: other.to_string(),
                detail,
            }]
        }
    }
}

fn parse_assistant(v: &Value, meta: &mut LineMeta) -> Vec<TranscriptItem> {
    let content = match v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        Some(arr) => arr,
        None => return vec![],
    };

    let mut items = Vec::new();
    for block in content {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match block_type {
            "text" => {
                let text = block
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                meta.stats_delta.assistant_chars += text.len();
                items.push(TranscriptItem::AssistantText {
                    text,
                    is_partial: false,
                });
            }
            "tool_use" => {
                let name = block
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let input = block.get("input").cloned();
                let summary = extract_tool_summary(&name, input.as_ref());
                meta.stats_delta.tool_calls += 1;
                items.push(TranscriptItem::ToolUse {
                    name,
                    summary,
                    input,
                });
            }
            "thinking" => {
                let text = block
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                if !text.is_empty() {
                    items.push(TranscriptItem::Thinking { text });
                }
            }
            _ => {
                // skip unknown block types
            }
        }
    }
    items
}

pub fn extract_tool_summary(name: &str, input: Option<&Value>) -> String {
    let Some(input) = input else {
        return name.to_string();
    };
    match name {
        "Read" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                return path.to_string();
            }
        }
        "Bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                return truncate_str(cmd, 80).to_string();
            }
        }
        "Edit" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                let old = input.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
                let new = input.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
                let removed = old.lines().count();
                let added = new.lines().count();
                return format!("{path}  +{added} -{removed}");
            }
        }
        "Write" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                return path.to_string();
            }
        }
        "Grep" => {
            if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                return pattern.to_string();
            }
        }
        "Glob" => {
            if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                return pattern.to_string();
            }
        }
        _ => {}
    }
    // Fallback: first key=value from input object
    if let Some(obj) = input.as_object()
        && let Some((key, val)) = obj.iter().next()
    {
        let val_str = val.as_str().unwrap_or("");
        if !val_str.is_empty() {
            return format!("{key}={val_str}");
        }
        return format!("{key}={val}");
    }
    name.to_string()
}

fn parse_user(v: &Value) -> Vec<TranscriptItem> {
    // Check for tool_use_result enrichment field
    let Some(tur) = v.get("tool_use_result") else {
        return vec![];
    };

    // Determine tool_name from enrichment
    let tool_name = if tur.get("file").is_some() {
        "Read".to_string()
    } else {
        // Try to infer from other context
        tur.get("tool_name")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown")
            .to_string()
    };

    // Extract summary: file path or truncated content
    let summary = tur
        .get("file")
        .and_then(|f| f.get("filePath"))
        .and_then(|p| p.as_str())
        .map(String::from)
        .or_else(|| {
            // Try to get content from the message's tool_result
            v.get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| {
                    arr.iter().find(|block| {
                        block.get("type").and_then(|t| t.as_str()) == Some("tool_result")
                    })
                })
                .and_then(|block| block.get("content"))
                .and_then(|c| c.as_str())
                .map(|s| truncate_str(s, 80).to_string())
        })
        .unwrap_or_else(|| tool_name.clone());

    // Extract content from message.content[].content where type=tool_result
    let content = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|block| block.get("type").and_then(|t| t.as_str()) == Some("tool_result"))
        })
        .and_then(|block| block.get("content"))
        .and_then(|c| c.as_str())
        .map(String::from);

    vec![TranscriptItem::ToolResult {
        tool_name,
        summary,
        content,
        duration_ms: None,
    }]
}

fn parse_result_event(v: &Value, meta: &mut LineMeta) -> Vec<TranscriptItem> {
    let result = SessionResult {
        subtype: v
            .get("subtype")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string(),
        is_error: v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false),
        duration_ms: v.get("duration_ms").and_then(|n| n.as_u64()).unwrap_or(0),
        num_turns: v.get("num_turns").and_then(|n| n.as_u64()).unwrap_or(0),
        total_cost_usd: v
            .get("total_cost_usd")
            .and_then(|n| n.as_f64())
            .unwrap_or(0.0),
        stop_reason: v
            .get("stop_reason")
            .and_then(|s| s.as_str())
            .map(String::from),
        result_text: v.get("result").and_then(|s| s.as_str()).map(String::from),
    };

    if let Some(usage) = v.get("usage") {
        let input = usage
            .get("input_tokens")
            .and_then(|n| n.as_u64())
            .unwrap_or(0);
        let output = usage
            .get("output_tokens")
            .and_then(|n| n.as_u64())
            .unwrap_or(0);
        let total = input + output;
        if total > 0 {
            meta.stats_delta.token_count = total;
        }
    }
    meta.stats_delta.num_turns = result.num_turns;

    let item = TranscriptItem::RunResult {
        is_error: result.is_error,
        stop_reason: result.stop_reason.clone(),
        num_turns: result.num_turns,
        total_cost_usd: result.total_cost_usd,
        duration_ms: result.duration_ms,
        result_text: result.result_text.clone(),
    };

    meta.stats_delta.cost_usd = Some(result.total_cost_usd);
    meta.session_result = Some(result);
    vec![item]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_system_init() {
        let line = r#"{"type":"system","subtype":"init","model":"claude-opus-4-6","tools":["Read","Edit","Bash"],"session_id":"abc-123","uuid":"u1"}"#;
        let ParseResult::Parsed(items, meta) = parse_line(line) else {
            panic!("expected Parsed")
        };
        assert_eq!(items.len(), 1);
        assert!(
            matches!(&items[0], TranscriptItem::SessionStart { model, .. } if model == "claude-opus-4-6")
        );
        assert_eq!(meta.session_id, Some("abc-123".to_string()));
    }

    #[test]
    fn parse_assistant_text() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello world"}],"role":"assistant"},"session_id":"abc"}"#;
        let ParseResult::Parsed(items, _) = parse_line(line) else {
            panic!("expected Parsed")
        };
        assert_eq!(items.len(), 1);
        assert!(
            matches!(&items[0], TranscriptItem::AssistantText { text, is_partial } if text == "Hello world" && !is_partial)
        );
    }

    #[test]
    fn parse_assistant_tool_use() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","id":"t1","input":{"file_path":"/foo/bar.rs"}}],"role":"assistant"},"session_id":"abc"}"#;
        let ParseResult::Parsed(items, _) = parse_line(line) else {
            panic!("expected Parsed")
        };
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], TranscriptItem::ToolUse { name, .. } if name == "Read"));
    }

    #[test]
    fn parse_user_tool_result() {
        let line = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"file contents"}]},"tool_use_result":{"type":"text","file":{"filePath":"/foo/bar.rs"}},"session_id":"abc"}"#;
        let ParseResult::Parsed(items, _) = parse_line(line) else {
            panic!("expected Parsed")
        };
        assert_eq!(items.len(), 1);
        // tool_name should be inferred — at minimum non-empty
        assert!(matches!(&items[0], TranscriptItem::ToolResult { .. }));
    }

    #[test]
    fn parse_result_success() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":60000,"num_turns":10,"total_cost_usd":2.5,"stop_reason":"end_turn","result":"Done","session_id":"abc"}"#;
        let ParseResult::Parsed(items, meta) = parse_line(line) else {
            panic!("expected Parsed")
        };
        assert_eq!(items.len(), 1);
        assert!(meta.session_result.is_some());
        assert_eq!(meta.session_result.unwrap().total_cost_usd, 2.5);
    }

    #[test]
    fn parse_stream_event_skipped() {
        let line =
            r#"{"type":"stream_event","event":{"type":"content_block_delta"},"session_id":"abc"}"#;
        assert!(matches!(parse_line(line), ParseResult::Skipped));
    }

    #[test]
    fn parse_malformed_json() {
        assert!(matches!(parse_line("not json {{{"), ParseResult::Error(_)));
    }

    #[test]
    fn parse_unknown_type() {
        let line = r#"{"type":"something_new","data":"hello","session_id":"abc"}"#;
        let ParseResult::Parsed(items, _) = parse_line(line) else {
            panic!("expected Parsed")
        };
        assert!(
            matches!(&items[0], TranscriptItem::SystemEvent { label, .. } if label == "something_new")
        );
    }

    #[test]
    fn parse_subagent_started() {
        let line = r#"{"type":"system","subtype":"task_started","task_id":"t1","description":"Do the thing","session_id":"abc"}"#;
        let ParseResult::Parsed(items, _) = parse_line(line) else {
            panic!("expected Parsed")
        };
        assert!(
            matches!(&items[0], TranscriptItem::SubagentStart { description, .. } if description == "Do the thing")
        );
    }

    #[test]
    fn parse_subagent_notification() {
        let line = r#"{"type":"system","subtype":"task_notification","task_id":"t1","status":"completed","summary":"Did the thing","usage":{"total_tokens":1000,"tool_uses":5,"duration_ms":30000},"session_id":"abc"}"#;
        let ParseResult::Parsed(items, _) = parse_line(line) else {
            panic!("expected Parsed")
        };
        assert!(
            matches!(&items[0], TranscriptItem::SubagentEnd { status, .. } if status == "completed")
        );
    }

    #[test]
    fn parse_assistant_thinking_block() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"thinking","text":"Let me think about this..."},{"type":"text","text":"Here is my answer"}],"role":"assistant"},"session_id":"abc"}"#;
        let ParseResult::Parsed(items, _) = parse_line(line) else {
            panic!("expected Parsed")
        };
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], TranscriptItem::Thinking { text } if text == "Let me think about this..."));
        assert!(matches!(&items[1], TranscriptItem::AssistantText { text, .. } if text == "Here is my answer"));
    }

    #[test]
    fn parse_result_emits_run_result_item() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":60000,"num_turns":10,"total_cost_usd":2.5,"stop_reason":"end_turn","result":"All done","session_id":"abc"}"#;
        let ParseResult::Parsed(items, meta) = parse_line(line) else {
            panic!("expected Parsed")
        };
        assert_eq!(items.len(), 1);
        assert!(matches!(
            &items[0],
            TranscriptItem::RunResult {
                is_error: false,
                num_turns: 10,
                total_cost_usd,
                duration_ms: 60000,
                ..
            } if (*total_cost_usd - 2.5).abs() < f64::EPSILON
        ));
        assert!(meta.session_result.is_some());
    }

    #[test]
    fn parse_subagent_notification_with_usage() {
        let line = r#"{"type":"system","subtype":"task_notification","task_id":"t1","status":"completed","summary":"Did the thing","usage":{"total_tokens":50000,"tool_uses":9,"duration_ms":240000},"session_id":"abc"}"#;
        let ParseResult::Parsed(items, _) = parse_line(line) else {
            panic!("expected Parsed")
        };
        assert!(matches!(
            &items[0],
            TranscriptItem::SubagentEnd {
                duration_ms: Some(240000),
                tool_uses: Some(9),
                total_tokens: Some(50000),
                ..
            }
        ));
    }
}
