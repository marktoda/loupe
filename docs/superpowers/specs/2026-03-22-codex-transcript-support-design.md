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

**Empty files:** If the file is empty when discovered, defer format detection to the first incremental parse that has content. `WatchedFile` stores `Option<Box<dyn TranscriptParser>>` and lazily initializes on first non-empty read.

### 3. New TranscriptItem Variant

Add `UserMessage { text: String }` to the `TranscriptItem` enum. Covers Codex `event_msg(user_message)` events. Could also be used for Claude Code in the future.

### 4. Codex Event Mapping

| Codex Event | TranscriptItem | Notes |
|---|---|---|
| `session_meta` | `SessionStart` | Extract session id, cwd, cli_version; model may be empty |
| `turn_context` | `SessionStart` | Extract model name; first occurrence only |
| `event_msg(user_message)` | `UserMessage` | User prompt text |
| `event_msg(agent_message)` | `AssistantText` | Commentary and final outputs |
| `event_msg(task_complete)` | `RunResult` | Session end |
| `event_msg(task_started)` | `SystemEvent` | Turn start marker |
| `event_msg(token_count)` | Skipped | Noise |
| `response_item(message)` + `output_text` | `AssistantText` | Model text output; iterate content array, skip `input_text` entries |
| `response_item(message)` + `input_text` only | Skipped | System context noise |
| `response_item(reasoning)` | Skipped | Encrypted, not readable |
| `response_item(function_call)` | `ToolUse` | Deserialize `arguments` JSON string into Value, then call `extract_tool_summary` |
| `response_item(function_call_output)` | `ToolResult` | Sequential ordering sufficient, no call_id pairing needed |
| `response_item(custom_tool_call)` | `ToolUse` + `ToolResult` | Self-contained (has input + status) |

**Tool summary extraction:** `extract_tool_summary` in `parser.rs` must be made `pub` so the Codex parser can reuse it. The Codex parser deserializes the `arguments` JSON string into a `serde_json::Value` before calling it.

**Stats:** The Codex parser should NOT populate `stats_delta` in `LineMeta` — let the watcher's `process_parsed_line` handle all stats counting to avoid double-counting (an existing issue in the Claude Code path).

**Known gaps:** Codex error events and approval/permission events are not mapped. These can be added later as `SystemEvent` or `Error` variants if needed.

### 5. Watcher Changes

`WatchedFile` gains two fields:
- `format: Option<Format>` — for format-specific decisions (e.g., streaming)
- `parser: Option<Box<dyn TranscriptParser>>` — lazily initialized on first non-empty read

On file discovery, the watcher reads the first line, detects format, constructs the appropriate parser. All subsequent lines go through the stored parser.

**Streaming:** The `stream_event` interception in `parse_file_incremental` is Claude Code-specific (Codex does not use this protocol). The watcher checks `format` before attempting stream event processing — only `Format::ClaudeCode` enters the streaming path.

No changes to file discovery — it watches `.jsonl` files in whatever directory it's given.

### 6. UI Changes

`transcript.rs` gets a new rendering arm for `UserMessage`:
- Label: `USER`
- Color: distinct from assistant (e.g., cyan/blue)
- Content: soft-wrapped text, same as `AssistantText`
- Not expandable (user messages are short)

**Search:** Add `UserMessage` arm to `recompute_search` in `app.rs` so user messages are searchable.

No changes to `RunStats` for user messages.

## Files Changed

1. **`src/parser.rs`** — Extract `TranscriptParser` trait, `Format` enum, `detect_format()`. Make `extract_tool_summary` pub. Refactor existing code into `ClaudeCodeParser`.
2. **New `src/codex_parser.rs`** — `CodexParser` implementing the trait.
3. **`src/run.rs`** — Add `UserMessage { text: String }` to `TranscriptItem`.
4. **`src/watcher.rs`** — Store `Format` + `Box<dyn TranscriptParser>` per file, detect format on discovery, gate streaming on format.
5. **`src/ui/transcript.rs`** — Render `UserMessage` with `USER` label.
6. **`src/app.rs`** — Add `UserMessage` to search match.
7. **`src/lib.rs`** — Add `pub mod codex_parser`.
8. **`tests/parser_integration_test.rs`** — Update to use `ClaudeCodeParser` struct method instead of free function.
9. **New test fixture** — Codex `.jsonl` file for parser tests.
