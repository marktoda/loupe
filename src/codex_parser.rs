use chrono::Utc;
use serde_json::Value;

use crate::parser::{extract_tool_summary, parse_timestamp, LineMeta, ParseResult, TranscriptParser, truncate_str};
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

        meta.timestamp = parse_timestamp(&v, "timestamp");

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

    let timestamp = parse_timestamp(payload, "timestamp").unwrap_or_else(Utc::now);
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
        _ => {}
    }
    // Fall through to shared summary extraction (first-key heuristic)
    extract_tool_summary(name, Some(args))
}

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
