# Codex Transcript Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parse and display Codex CLI JSONL transcripts alongside existing Claude Code transcripts with automatic format detection.

**Architecture:** Introduce a `TranscriptParser` trait with `ClaudeCodeParser` and `CodexParser` implementations. The watcher auto-detects format from the first JSONL line (Codex has `"payload"` key, Claude Code does not) and stores the appropriate parser per file. A new `UserMessage` variant in `TranscriptItem` renders Codex user messages.

**Tech Stack:** Rust, ratatui, serde_json, chrono

**Spec:** `docs/superpowers/specs/2026-03-22-codex-transcript-support-design.md`

---

## File Structure

| File | Responsibility |
|---|---|
| `src/parser.rs` | `TranscriptParser` trait, `Format` enum, `detect_format()`, `ClaudeCodeParser`, shared helpers (made `pub`) |
| `src/codex_parser.rs` | `CodexParser` struct implementing `TranscriptParser` |
| `src/run.rs` | `TranscriptItem` enum (add `UserMessage` variant) |
| `src/watcher.rs` | Per-file format detection, `Box<dyn TranscriptParser>` storage, streaming gating |
| `src/app.rs` | Search support for `UserMessage` |
| `src/ui/transcript.rs` | `USER` label rendering |
| `src/lib.rs` | `pub mod codex_parser` |
| `tests/fixtures/codex_session.jsonl` | Test fixture with representative Codex events |
| `tests/parser_integration_test.rs` | Update to use trait, add Codex fixture test |

---

### Task 1: Add `UserMessage` variant to `TranscriptItem`

**Files:**
- Modify: `src/run.rs:31-86`

- [ ] **Step 1: Add the variant**

In `src/run.rs`, add `UserMessage` to the `TranscriptItem` enum after `RunResult`:

```rust
    RunResult {
        is_error: bool,
        stop_reason: Option<String>,
        num_turns: u64,
        total_cost_usd: f64,
        duration_ms: u64,
        result_text: Option<String>,
    },
    UserMessage {
        text: String,
    },
```

- [ ] **Step 2: Handle exhaustive matches**

Add `UserMessage` arms to all existing `match` statements:

In `src/app.rs:339` (inside `recompute_search`), before the `_ => false` arm:
```rust
TranscriptItem::UserMessage { text } => {
    text.to_lowercase().contains(&query_lower)
}
```

In `src/watcher.rs:51-57` (inside `process_parsed_line`), add to the match:
```rust
TranscriptItem::UserMessage { .. } => {}
```

In `src/ui/transcript.rs:133` (inside the `for item in &run.items` loop), add before the closing brace:
```rust
TranscriptItem::UserMessage { text } => {
    for (li, source_line) in text.lines().enumerate() {
        let wrapped = soft_wrap(source_line, content_cols);
        for (wi, chunk) in wrapped.iter().enumerate() {
            let content_spans = if search_active {
                highlight_text(chunk, &query, default)
            } else {
                vec![Span::styled(chunk.to_string(), default)]
            };
            let prefix = if li == 0 && wi == 0 {
                Span::styled("USER     ", label_bold(Color::Blue))
            } else {
                Span::raw("         ")
            };
            let mut spans = vec![prefix];
            spans.extend(content_spans);
            lines.push(Line::from(spans));
        }
    }
    lines.push(Line::default());
}
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles cleanly with no warnings about non-exhaustive matches.

- [ ] **Step 4: Commit**

```bash
git add src/run.rs src/app.rs src/watcher.rs src/ui/transcript.rs
git commit -m "feat: add UserMessage variant to TranscriptItem"
```

---

### Task 2: Extract `TranscriptParser` trait and `ClaudeCodeParser`

**Files:**
- Modify: `src/parser.rs:1-83`

- [ ] **Step 1: Add the trait and Format enum**

At the top of `src/parser.rs` (after imports, before `LineMeta`), add:

```rust
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
```

Note: `Send + Sync` are required because `Box<dyn TranscriptParser>` is stored in `WatchedFile` which is used across the async boundary.

- [ ] **Step 2: Add `detect_format` function**

```rust
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
```

- [ ] **Step 3: Create `ClaudeCodeParser` struct implementing the trait**

```rust
/// Parser for Claude Code's JSONL stream format.
pub struct ClaudeCodeParser;

impl TranscriptParser for ClaudeCodeParser {
    fn parse_line(&self, line: &str) -> ParseResult {
        parse_line(line)
    }
}
```

The existing `parse_line` free function stays as-is (it's used by tests and internally). The struct delegates to it.

- [ ] **Step 4: Make `extract_tool_summary` and `truncate_str` public**

Change the function signatures:
- `fn extract_tool_summary(` → `pub fn extract_tool_summary(`
- `truncate_str` is already `pub`

- [ ] **Step 5: Build and run existing tests**

Run: `cargo test 2>&1`
Expected: All existing tests pass. The free `parse_line` function still exists, so no call sites break.

- [ ] **Step 6: Commit**

```bash
git add src/parser.rs
git commit -m "feat: add TranscriptParser trait, Format enum, ClaudeCodeParser"
```

---

### Task 3: Create Codex test fixture

**Files:**
- Create: `tests/fixtures/codex_session.jsonl`

- [ ] **Step 1: Create the fixture file**

Create `tests/fixtures/codex_session.jsonl` with representative Codex events. Each line must be valid JSON matching the real Codex schema. Include:

```jsonl
{"timestamp":"2026-03-22T19:54:06.506Z","type":"session_meta","payload":{"id":"sess-codex-001","timestamp":"2026-03-22T19:54:06.501Z","cwd":"/home/user/project","originator":"codex_cli_rs","cli_version":"0.116.0","source":"cli","model_provider":"openai","base_instructions":{"text":"System prompt"},"git":{"commit_hash":"abc123","branch":"main","repository_url":"git@github.com:user/repo.git"}}}
{"timestamp":"2026-03-22T19:54:06.507Z","type":"turn_context","payload":{"turn_id":"turn-001","cwd":"/home/user/project","current_date":"2026-03-22","timezone":"America/New_York","approval_policy":"on-request","sandbox_policy":{"type":"workspace-write","writable_roots":[],"network_access":false},"model":"gpt-5.4","personality":"pragmatic","collaboration_mode":{"mode":"default","settings":{"model":"gpt-5.4"}}}}
{"timestamp":"2026-03-22T19:54:06.507Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn-001","model_context_window":258400,"collaboration_mode_kind":"default"}}
{"timestamp":"2026-03-22T19:54:06.507Z","type":"event_msg","payload":{"type":"user_message","message":"Please read the README and run the tests"}}
{"timestamp":"2026-03-22T19:54:06.507Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"limit_id":"codex","primary":{"used_percent":0.0,"window_minutes":300,"resets_at":1774227062}}}}
{"timestamp":"2026-03-22T19:54:06.507Z","type":"response_item","payload":{"type":"reasoning","summary":[],"content":null,"encrypted_content":"gAAAA_encrypted_data"}}
{"timestamp":"2026-03-22T19:54:06.507Z","type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"System permissions and context"}]}}
{"timestamp":"2026-03-22T19:54:06.507Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context><cwd>/home/user/project</cwd></environment_context>"}]}}
{"timestamp":"2026-03-22T19:54:06.507Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"I'll read the README and then run the tests."}],"phase":"commentary"}}
{"timestamp":"2026-03-22T19:54:08.000Z","type":"event_msg","payload":{"type":"agent_message","message":"Reading project files now.","phase":"commentary","memory_citation":null}}
{"timestamp":"2026-03-22T19:54:09.000Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"cat README.md\",\"workdir\":\"/home/user/project\"}","call_id":"call_001"}}
{"timestamp":"2026-03-22T19:54:09.500Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call_001","output":"# My Project\nA test project."}}
{"timestamp":"2026-03-22T19:54:10.000Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"cargo test\",\"workdir\":\"/home/user/project\"}","call_id":"call_002"}}
{"timestamp":"2026-03-22T19:54:15.000Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call_002","output":"running 5 tests\ntest result: ok. 5 passed"}}
{"timestamp":"2026-03-22T19:54:16.000Z","type":"response_item","payload":{"type":"custom_tool_call","status":"completed","call_id":"call_003","name":"apply_patch","input":"*** Begin Patch\n*** Update File: src/main.rs\n@@\n-old line\n+new line\n"}}
{"timestamp":"2026-03-22T19:54:17.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"All tests pass. I've also applied a small fix."}],"phase":"final"}}
{"timestamp":"2026-03-22T19:54:18.000Z","type":"event_msg","payload":{"type":"agent_message","message":"All tests pass. Applied the fix.","phase":"final","memory_citation":null}}
{"timestamp":"2026-03-22T19:54:19.000Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-001","last_agent_message":"All tests pass. Applied the fix."}}
```

- [ ] **Step 2: Commit**

```bash
git add tests/fixtures/codex_session.jsonl
git commit -m "test: add Codex session fixture"
```

---

### Task 4: Implement `CodexParser`

**Files:**
- Create: `src/codex_parser.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write tests first**

Create `src/codex_parser.rs` with the full module. Start with tests at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TranscriptParser;

    #[test]
    fn parse_session_meta() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:06.506Z","type":"session_meta","payload":{"id":"sess-001","timestamp":"2026-03-22T19:54:06.501Z","cwd":"/home/user/project","originator":"codex_cli_rs","cli_version":"0.116.0","source":"cli","model_provider":"openai","base_instructions":{"text":"prompt"}}}"#;
        let ParseResult::Parsed(items, meta) = parser.parse_line(line) else {
            panic!("expected Parsed");
        };
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], TranscriptItem::SessionStart { model, .. } if model.is_empty()));
        assert_eq!(meta.session_id, Some("sess-001".to_string()));
    }

    #[test]
    fn parse_turn_context_extracts_model() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:06.507Z","type":"turn_context","payload":{"turn_id":"t1","model":"gpt-5.4","cwd":"/tmp"}}"#;
        let ParseResult::Parsed(items, _) = parser.parse_line(line) else {
            panic!("expected Parsed");
        };
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], TranscriptItem::SessionStart { model, .. } if model == "gpt-5.4"));
    }

    #[test]
    fn parse_user_message() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:06.507Z","type":"event_msg","payload":{"type":"user_message","message":"Hello world"}}"#;
        let ParseResult::Parsed(items, _) = parser.parse_line(line) else {
            panic!("expected Parsed");
        };
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], TranscriptItem::UserMessage { text } if text == "Hello world"));
    }

    #[test]
    fn parse_agent_message() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:06.507Z","type":"event_msg","payload":{"type":"agent_message","message":"Working on it.","phase":"commentary"}}"#;
        let ParseResult::Parsed(items, _) = parser.parse_line(line) else {
            panic!("expected Parsed");
        };
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], TranscriptItem::AssistantText { text, .. } if text == "Working on it."));
    }

    #[test]
    fn parse_function_call() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:09.000Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"cat README.md\",\"workdir\":\"/tmp\"}","call_id":"call_001"}}"#;
        let ParseResult::Parsed(items, _) = parser.parse_line(line) else {
            panic!("expected Parsed");
        };
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], TranscriptItem::ToolUse { name, .. } if name == "exec_command"));
    }

    #[test]
    fn parse_function_call_output() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:09.500Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call_001","output":"file contents"}}"#;
        let ParseResult::Parsed(items, _) = parser.parse_line(line) else {
            panic!("expected Parsed");
        };
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], TranscriptItem::ToolResult { .. }));
    }

    #[test]
    fn parse_custom_tool_call() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:16.000Z","type":"response_item","payload":{"type":"custom_tool_call","status":"completed","call_id":"call_003","name":"apply_patch","input":"*** Begin Patch\n*** Update File: src/main.rs\n@@\n-old\n+new\n"}}"#;
        let ParseResult::Parsed(items, _) = parser.parse_line(line) else {
            panic!("expected Parsed");
        };
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], TranscriptItem::ToolUse { name, .. } if name == "apply_patch"));
        assert!(matches!(&items[1], TranscriptItem::ToolResult { .. }));
    }

    #[test]
    fn parse_assistant_output_text() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:06.507Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hello from assistant"}],"phase":"commentary"}}"#;
        let ParseResult::Parsed(items, _) = parser.parse_line(line) else {
            panic!("expected Parsed");
        };
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], TranscriptItem::AssistantText { text, .. } if text == "Hello from assistant"));
    }

    #[test]
    fn skip_reasoning() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:06.507Z","type":"response_item","payload":{"type":"reasoning","summary":[],"content":null,"encrypted_content":"gAAAA"}}"#;
        assert!(matches!(parser.parse_line(line), ParseResult::Skipped));
    }

    #[test]
    fn skip_developer_message() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:06.507Z","type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"system stuff"}]}}"#;
        assert!(matches!(parser.parse_line(line), ParseResult::Skipped));
    }

    #[test]
    fn skip_user_input_text() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:06.507Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<env_context/>"}]}}"#;
        assert!(matches!(parser.parse_line(line), ParseResult::Skipped));
    }

    #[test]
    fn skip_token_count() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:06.507Z","type":"event_msg","payload":{"type":"token_count","info":null}}"#;
        assert!(matches!(parser.parse_line(line), ParseResult::Skipped));
    }

    #[test]
    fn parse_task_started() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:06.507Z","type":"event_msg","payload":{"type":"task_started","turn_id":"t1","model_context_window":258400}}"#;
        let ParseResult::Parsed(items, _) = parser.parse_line(line) else {
            panic!("expected Parsed");
        };
        assert!(matches!(&items[0], TranscriptItem::SystemEvent { label, .. } if label == "task_started"));
    }

    #[test]
    fn parse_task_complete() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:19.000Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"t1","last_agent_message":"Done."}}"#;
        let ParseResult::Parsed(items, meta) = parser.parse_line(line) else {
            panic!("expected Parsed");
        };
        assert!(matches!(&items[0], TranscriptItem::RunResult { .. }));
        assert!(meta.session_result.is_some());
    }

    #[test]
    fn parse_exec_command_summary() {
        let parser = CodexParser;
        let line = r#"{"timestamp":"2026-03-22T19:54:09.000Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"ls -la /tmp\",\"workdir\":\"/tmp\"}","call_id":"call_001"}}"#;
        let ParseResult::Parsed(items, _) = parser.parse_line(line) else {
            panic!("expected Parsed");
        };
        assert!(matches!(&items[0], TranscriptItem::ToolUse { summary, .. } if summary == "ls -la /tmp"));
    }
}
```

- [ ] **Step 2: Write the implementation**

**IMPORTANT:** Helpers return `Vec<TranscriptItem>` (not `ParseResult`), because `ParseResult::Parsed` takes an owned `LineMeta` — you can't move it out of `&mut`. The top-level `parse_line` owns `meta` and wraps results, matching the Claude Code parser pattern.

`parse_event_msg` and `parse_response_item` return `Option<Vec<TranscriptItem>>` where `None` means skip.

```rust
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::parser::{extract_tool_summary, LineMeta, ParseResult, TranscriptParser, truncate_str};
use crate::run::{SessionResult, TranscriptItem};

/// Parser for Codex CLI's JSONL transcript format.
/// Codex wraps every event in `{ timestamp, type, payload }`.
pub struct CodexParser;

impl TranscriptParser for CodexParser {
    fn parse_line(&self, line: &str) -> ParseResult {
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return ParseResult::Error(format!("JSON parse error: {e}")),
        };

        let Some(event_type) = v.get("type").and_then(|t| t.as_str()) else {
            return ParseResult::Error("Missing 'type' field".to_string());
        };

        let payload = match v.get("payload") {
            Some(p) => p,
            None => return ParseResult::Error("Missing 'payload' field".to_string()),
        };

        let mut meta = LineMeta::default();

        // Extract timestamp from top-level
        meta.timestamp = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        match event_type {
            "session_meta" => {
                let items = parse_session_meta(payload, &mut meta);
                ParseResult::Parsed(items, meta)
            }
            "turn_context" => {
                let items = parse_turn_context(payload, &mut meta);
                ParseResult::Parsed(items, meta)
            }
            "event_msg" => match parse_event_msg(payload, &mut meta) {
                Some(items) => ParseResult::Parsed(items, meta),
                None => ParseResult::Skipped,
            },
            "response_item" => match parse_response_item(payload, &mut meta) {
                Some(items) => ParseResult::Parsed(items, meta),
                None => ParseResult::Skipped,
            },
            _ => ParseResult::Parsed(
                vec![TranscriptItem::SystemEvent {
                    label: event_type.to_string(),
                    detail: serde_json::to_string(&v).unwrap_or_default(),
                }],
                meta,
            ),
        }
    }
}

fn parse_session_meta(payload: &Value, meta: &mut LineMeta) -> Vec<TranscriptItem> {
    meta.session_id = payload.get("id").and_then(|s| s.as_str()).map(String::from);

    let timestamp = payload
        .get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    meta.timestamp = Some(timestamp);

    vec![TranscriptItem::SessionStart {
        model: String::new(),
        tools: Vec::new(),
        timestamp,
    }]
}

fn parse_turn_context(payload: &Value, meta: &mut LineMeta) -> Vec<TranscriptItem> {
    let model = payload
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();

    let timestamp = meta.timestamp.unwrap_or_else(Utc::now);

    vec![TranscriptItem::SessionStart {
        model,
        tools: Vec::new(),
        timestamp,
    }]
}

/// Returns `None` to signal ParseResult::Skipped.
fn parse_event_msg(payload: &Value, meta: &mut LineMeta) -> Option<Vec<TranscriptItem>> {
    let msg_type = payload
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("");

    match msg_type {
        "user_message" => {
            let message = payload
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            Some(vec![TranscriptItem::UserMessage { text: message }])
        }
        "agent_message" => {
            let message = payload
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            Some(vec![TranscriptItem::AssistantText {
                text: message,
                is_partial: false,
            }])
        }
        "task_started" => Some(vec![TranscriptItem::SystemEvent {
            label: "task_started".to_string(),
            detail: payload
                .get("collaboration_mode_kind")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
        }]),
        "task_complete" => {
            let result = SessionResult {
                subtype: "success".to_string(),
                is_error: false,
                duration_ms: 0,
                num_turns: 0,
                total_cost_usd: 0.0,
                stop_reason: Some("task_complete".to_string()),
                result_text: payload
                    .get("last_agent_message")
                    .and_then(|s| s.as_str())
                    .map(String::from),
            };
            let item = TranscriptItem::RunResult {
                is_error: false,
                stop_reason: Some("task_complete".to_string()),
                num_turns: 0,
                total_cost_usd: 0.0,
                duration_ms: 0,
                result_text: result.result_text.clone(),
            };
            meta.session_result = Some(result);
            Some(vec![item])
        }
        "token_count" => None,
        _ => Some(vec![TranscriptItem::SystemEvent {
            label: msg_type.to_string(),
            detail: serde_json::to_string(payload).unwrap_or_default(),
        }]),
    }
}

/// Returns `None` to signal ParseResult::Skipped.
fn parse_response_item(payload: &Value, _meta: &mut LineMeta) -> Option<Vec<TranscriptItem>> {
    let item_type = payload
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("");

    match item_type {
        "reasoning" => None,

        "message" => parse_message(payload),

        "function_call" => {
            let name = payload
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();

            let args_str = payload
                .get("arguments")
                .and_then(|a| a.as_str())
                .unwrap_or("{}");
            let args_value: Option<Value> = serde_json::from_str(args_str).ok();

            let summary = extract_codex_tool_summary(&name, args_value.as_ref());
            let input = args_value;

            Some(vec![TranscriptItem::ToolUse {
                name,
                summary,
                input,
            }])
        }

        "function_call_output" => {
            let output = payload
                .get("output")
                .and_then(|o| o.as_str())
                .unwrap_or("")
                .to_string();
            let summary = truncate_str(&output, 80).to_string();

            Some(vec![TranscriptItem::ToolResult {
                tool_name: String::new(),
                summary,
                content: Some(output),
                duration_ms: None,
            }])
        }

        "custom_tool_call" => {
            let name = payload
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let input_text = payload
                .get("input")
                .and_then(|i| i.as_str())
                .unwrap_or("")
                .to_string();
            let status = payload
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("unknown")
                .to_string();

            let summary = input_text
                .lines()
                .next()
                .unwrap_or(&name);
            let summary = truncate_str(summary, 80).to_string();

            Some(vec![
                TranscriptItem::ToolUse {
                    name: name.clone(),
                    summary: summary.clone(),
                    input: None,
                },
                TranscriptItem::ToolResult {
                    tool_name: name,
                    summary: status,
                    content: Some(input_text),
                    duration_ms: None,
                },
            ])
        }

        _ => None,
    }
}

fn parse_message(payload: &Value) -> Option<Vec<TranscriptItem>> {
    let role = payload
        .get("role")
        .and_then(|r| r.as_str())
        .unwrap_or("");

    if role == "developer" || role == "user" {
        return None;
    }

    let content = payload.get("content").and_then(|c| c.as_array())?;

    let mut items = Vec::new();
    for block in content {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if block_type == "output_text" {
            let text = block
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            if !text.is_empty() {
                items.push(TranscriptItem::AssistantText {
                    text,
                    is_partial: false,
                });
            }
        }
    }

    if items.is_empty() { None } else { Some(items) }
}

/// Extract a human-readable summary for Codex tool calls.
/// Codex uses different tool names than Claude Code:
/// - `exec_command` with `cmd` field (like Bash)
/// - `apply_patch` with patch text (custom_tool_call)
/// - `spawn_agent` with agent details
/// - `update_plan` with plan steps
fn extract_codex_tool_summary(name: &str, args: Option<&Value>) -> String {
    let Some(args) = args else {
        return name.to_string();
    };
    match name {
        "exec_command" => {
            if let Some(cmd) = args.get("cmd").and_then(|c| c.as_str()) {
                return truncate_str(cmd, 80).to_string();
            }
        }
        "spawn_agent" => {
            if let Some(msg) = args.get("message").and_then(|m| m.as_str()) {
                return truncate_str(msg, 80).to_string();
            }
        }
        "update_plan" => {
            return "update plan".to_string();
        }
        _ => {
            // Try Claude Code-style summary extraction as fallback
            return extract_tool_summary(name, Some(args));
        }
    }
    name.to_string()
}
```

- [ ] **Step 3: Add module to lib.rs**

In `src/lib.rs`, add:
```rust
pub mod codex_parser;
```

- [ ] **Step 4: Run tests**

Run: `cargo test codex 2>&1`
Expected: All `codex_parser::tests::*` tests pass.

Run: `cargo test 2>&1`
Expected: All tests pass (existing + new).

- [ ] **Step 5: Commit**

```bash
git add src/codex_parser.rs src/lib.rs
git commit -m "feat: implement CodexParser with full event mapping"
```

---

### Task 5: Wire format detection into the watcher

**Files:**
- Modify: `src/watcher.rs`

- [ ] **Step 1: Update WatchedFile struct and imports**

In `src/watcher.rs`, update the imports and struct:

```rust
use crate::codex_parser::CodexParser;
use crate::parser::{self, ClaudeCodeParser, Format, TranscriptParser};
```

Update `WatchedFile` to use `Option` for lazy init (handles empty files per spec):
```rust
struct WatchedFile {
    run_id: usize,
    bytes_read: u64,
    has_result: bool,
    format: Option<Format>,
    parser: Option<Box<dyn TranscriptParser>>,
}
```

- [ ] **Step 2: Add helper to create parser from format**

```rust
fn make_parser(format: Format) -> Box<dyn TranscriptParser> {
    match format {
        Format::ClaudeCode => Box::new(ClaudeCodeParser),
        Format::Codex => Box::new(CodexParser),
    }
}

/// Detect format from first line and return (format, parser).
/// Falls back to ClaudeCode if detection fails.
fn detect_and_make_parser(first_line: &str) -> (Format, Box<dyn TranscriptParser>) {
    let format = parser::detect_format(first_line).unwrap_or(Format::ClaudeCode);
    (format, make_parser(format))
}
```

- [ ] **Step 3: Update `parse_file_initial` to detect format and use trait**

In `parse_file_initial`, after reading the content, detect format from the first line. If the file is empty, return with `None` parser (lazy init):

```rust
let content = std::fs::read_to_string(path)?;
let bytes_read = content.len() as u64;

// Empty file — defer format detection to incremental parse
let first_line = content.lines().next();
if first_line.is_none() {
    return Ok(WatchedFile {
        run_id,
        bytes_read: 0,
        has_result: false,
        format: None,
        parser: None,
    });
}

let (format, file_parser) = detect_and_make_parser(first_line.unwrap());
```

Replace `parser::parse_line(line)` with `file_parser.parse_line(line)`.

Update the returned `WatchedFile`:
```rust
Ok(WatchedFile {
    run_id,
    bytes_read,
    has_result,
    format: Some(format),
    parser: Some(file_parser),
})
```

- [ ] **Step 4: Update `parse_file_incremental` to use stored parser and gate streaming**

At the top of `parse_file_incremental`, if the parser is `None` (empty file discovered earlier), detect format from the first new line:

```rust
// Lazy init: detect format on first content if not yet known
if wf.parser.is_none() {
    if let Some(first_line) = new_content.lines().next() {
        let (format, file_parser) = detect_and_make_parser(first_line);
        wf.format = Some(format);
        wf.parser = Some(file_parser);
    } else {
        return Ok(()); // Still empty
    }
}
let file_parser = wf.parser.as_ref().unwrap();
```

In the streaming interception block, add a format check:
```rust
// For the active run, try Tier 2 streaming first (Claude Code only)
if is_active
    && wf.format == Some(Format::ClaudeCode)
    && let Ok(v) = serde_json::from_str::<serde_json::Value>(line)
    && v.get("type").and_then(|t| t.as_str()) == Some("stream_event")
{
    // ... existing streaming logic
}
```

Replace `parser::parse_line(line)` with `file_parser.parse_line(line)`.

- [ ] **Step 5: Build and run all tests**

Run: `cargo test 2>&1`
Expected: All tests pass, including the watcher integration test.

- [ ] **Step 6: Commit**

```bash
git add src/watcher.rs
git commit -m "feat: wire format detection into watcher with per-file parsers"
```

---

### Task 6: Update integration tests

**Files:**
- Modify: `tests/parser_integration_test.rs`

- [ ] **Step 1: Update existing test to use ClaudeCodeParser**

The existing test uses `loupe::parser::parse_line(line)` as a free function. This still works, but add a parallel test using the trait:

```rust
#[test]
fn parse_codex_session_fixture() {
    use loupe::codex_parser::CodexParser;
    use loupe::parser::TranscriptParser;

    let content = std::fs::read_to_string("tests/fixtures/codex_session.jsonl").unwrap();
    let parser = CodexParser;
    let mut items = Vec::new();
    let mut result = None;

    for line in content.lines() {
        if let loupe::parser::ParseResult::Parsed(new_items, meta) = parser.parse_line(line) {
            items.extend(new_items);
            if let Some(r) = meta.session_result {
                result = Some(r);
            }
        }
    }

    assert!(!items.is_empty(), "Should have parsed some items");
    assert!(
        items.iter().any(|i| matches!(i, loupe::run::TranscriptItem::SessionStart { .. })),
        "Should have SessionStart"
    );
    assert!(
        items.iter().any(|i| matches!(i, loupe::run::TranscriptItem::AssistantText { .. })),
        "Should have AssistantText"
    );
    assert!(
        items.iter().any(|i| matches!(i, loupe::run::TranscriptItem::ToolUse { .. })),
        "Should have ToolUse"
    );
    assert!(
        items.iter().any(|i| matches!(i, loupe::run::TranscriptItem::UserMessage { .. })),
        "Should have UserMessage"
    );
    assert!(result.is_some(), "Should have a session result");
}
```

- [ ] **Step 2: Add format detection test**

```rust
#[test]
fn detect_format_from_fixtures() {
    use loupe::parser::{detect_format, Format};

    let claude_first = std::fs::read_to_string("tests/fixtures/minimal_session.jsonl")
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_string();
    assert_eq!(detect_format(&claude_first), Some(Format::ClaudeCode));

    let codex_first = std::fs::read_to_string("tests/fixtures/codex_session.jsonl")
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_string();
    assert_eq!(detect_format(&codex_first), Some(Format::Codex));
}
```

- [ ] **Step 3: Run all tests**

Run: `cargo test 2>&1`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add tests/parser_integration_test.rs
git commit -m "test: add Codex integration tests and format detection test"
```

---

### Task 7: Manual smoke test with real Codex transcripts

- [ ] **Step 1: Build release**

Run: `cargo build --release 2>&1`

- [ ] **Step 2: Run against Codex sessions directory**

Run: `./target/release/loupe ~/.codex/sessions/2026/03/22/`

Verify:
- All 3 Codex sessions appear in the run list
- Session items show: SESSION (model), USER (prompts), ASSIST (agent messages), TOOL (exec_command, apply_patch), RESULT
- Scrolling, search (`/`), expand (`e`) all work
- No parse errors in the transcript

- [ ] **Step 3: Run against Claude Code sessions (regression)**

Point loupe at any existing Claude Code JSONL directory and verify it still works identically.

- [ ] **Step 4: Final commit if any fixes needed**

If smoke testing reveals issues, fix and commit.
