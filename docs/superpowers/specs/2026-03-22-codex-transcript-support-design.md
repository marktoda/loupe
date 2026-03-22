# Codex Transcript Support

**Date:** 2026-03-22
**Status:** Approved

## Goal

Extend loupe to parse and display Codex CLI transcripts alongside existing Claude Code transcripts. Format detection is automatic — the user points loupe at any directory containing `.jsonl` files and it just works.

## Format Differences

Claude Code JSONL has top-level `type` fields (`system`, `assistant`, `user`, `result`, `stream_event`).

Codex JSONL wraps everything in `{ timestamp, type, payload }` where `type` is one of `session_meta`, `turn_context`, `event_msg`, or `response_item`. Tool calls use `function_call`/`function_call_output`/`custom_tool_call` instead of content blocks.

## Design

### 1. Parser Trait

Extract a `TranscriptParser` trait from the existing parsing logic:

```rust
pub trait TranscriptParser {
    fn parse_line(&self, line: &str) -> ParseResult;
}
```

`ParseResult` stays as-is: `Parsed(Vec<TranscriptItem>, LineMeta)`, `Skipped`, or `Error(String)`.

The existing Claude Code logic becomes `ClaudeCodeParser` implementing this trait. A new `CodexParser` does the same. Both are stateless.

### 2. Format Detection

```rust
pub enum Format {
    ClaudeCode,
    Codex,
}

pub fn detect_format(first_line: &str) -> Option<Format>
```

Heuristic: Codex lines always have a `"payload"` key at the top level; Claude Code never does. Called once when a file is discovered. Falls back to `ClaudeCode` if detection fails.

### 3. New TranscriptItem Variant

Add `UserMessage { text: String }` to the `TranscriptItem` enum. Covers Codex `event_msg(user_message)` events. Could also be used for Claude Code in the future.

### 4. Codex Event Mapping

| Codex Event | TranscriptItem | Notes |
|---|---|---|
| `session_meta` | `SessionStart` | Model from `turn_context` |
| `event_msg(user_message)` | `UserMessage` | User prompt text |
| `event_msg(agent_message)` | `AssistantText` | Commentary and final outputs |
| `event_msg(task_complete)` | `RunResult` | Session end |
| `event_msg(task_started)` | `SystemEvent` | Turn start marker |
| `event_msg(token_count)` | Skipped | Noise |
| `response_item(message)` + `output_text` | `AssistantText` | Model text output |
| `response_item(message)` + `input_text` | Skipped | System context noise |
| `response_item(reasoning)` | Skipped | Encrypted, not readable |
| `response_item(function_call)` | `ToolUse` | Parse name + arguments JSON |
| `response_item(function_call_output)` | `ToolResult` | Paired via call_id |
| `response_item(custom_tool_call)` | `ToolUse` + `ToolResult` | Self-contained |
| `turn_context` | Skipped | Metadata only (model name extraction) |

Tool summary extraction reuses the same logic as Claude Code (Read → file path, Bash → command, etc.) by parsing the `arguments` JSON string.

### 5. Watcher Changes

`WatchedFile` gains a `parser: Box<dyn TranscriptParser>` field. On file discovery, the watcher reads the first line, detects format, constructs the appropriate parser. All subsequent lines go through the stored parser.

No changes to file discovery — it watches `.jsonl` files in whatever directory it's given.

### 6. UI Changes

`transcript.rs` gets a new rendering arm for `UserMessage`:
- Label: `USER`
- Color: distinct from assistant (e.g., cyan/blue)
- Content: soft-wrapped text, same as `AssistantText`
- Not expandable (user messages are short)

No changes to `RunStats` for user messages.

## Files Changed

1. **`src/parser.rs`** — Extract `TranscriptParser` trait, `Format` enum, `detect_format()`. Refactor existing code into `ClaudeCodeParser`.
2. **New `src/codex_parser.rs`** — `CodexParser` implementing the trait.
3. **`src/run.rs`** — Add `UserMessage { text: String }` to `TranscriptItem`.
4. **`src/watcher.rs`** — Store `Box<dyn TranscriptParser>` per file, detect format on discovery.
5. **`src/ui/transcript.rs`** — Render `UserMessage` with `USER` label.
6. **New test fixture** — Codex `.jsonl` file for parser tests.
