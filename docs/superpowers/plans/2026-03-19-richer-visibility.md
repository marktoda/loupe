# Richer Visibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a run summary pane, thinking block display, inline timing/stats, and a global expand/collapse toggle to loupe.

**Architecture:** Data model changes in `run.rs` land first (new TranscriptItem variants, RunStats fields). Parser updates emit the new items. App state simplifies from per-item HashSet to a global bool for expansion. A new `run_summary.rs` UI component renders stats below the run list. Transcript rendering adds Thinking, RunResult, tool timing, and subagent stats inline.

**Tech Stack:** Rust, ratatui, serde_json, chrono

**Spec:** `docs/superpowers/specs/2026-03-19-richer-visibility-design.md`

---

### Task 1: Data Model Changes

**Files:**
- Modify: `src/run.rs`

This task adds all new types and fields that downstream tasks depend on. No behavior changes yet.

- [ ] **Step 1: Write tests for new RunStats fields and merge behavior**

Add to the existing `mod tests` in `src/run.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test run_stats_merge_new_fields`
Expected: Compile error — `num_turns` and `token_count` don't exist on `RunStats`.

- [ ] **Step 3: Add new fields to RunStats and update merge**

In `src/run.rs`, add to `RunStats` struct (after `cost_usd` field at line 93):

```rust
pub num_turns: u64,
pub token_count: u64,
```

In `RunStats::merge` (line 123-133), add after the `cost_usd` block:

```rust
if other.num_turns > 0 {
    self.num_turns = other.num_turns;
}
if other.token_count > 0 {
    self.token_count = other.token_count;
}
```

These use replacement semantics (not additive) because `num_turns` and `token_count` come from the final `result` event which reports totals.

- [ ] **Step 4: Add new TranscriptItem variants**

In `src/run.rs`, add to the `TranscriptItem` enum (after `SystemEvent` variant, before the closing `}`):

```rust
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
```

- [ ] **Step 5: Add fields to SubagentEnd variant**

In `src/run.rs`, update the `SubagentEnd` variant (lines 60-64) to:

```rust
SubagentEnd {
    summary: String,
    status: String,
    cost_usd: Option<f64>,
    duration_ms: Option<u64>,
    tool_uses: Option<u64>,
    total_tokens: Option<u64>,
},
```

- [ ] **Step 6: Add duration_ms to ToolResult variant**

In `src/run.rs`, update the `ToolResult` variant (lines 47-51) to:

```rust
ToolResult {
    tool_name: String,
    summary: String,
    content: Option<String>,
    duration_ms: Option<u64>,
},
```

- [ ] **Step 7: Add tool_timestamps to Run struct**

Add `use std::time::Instant;` at the top of `src/run.rs` (with the other imports).

Add to `Run` struct (after `last_modified` field at line 19):

```rust
#[allow(dead_code)]
pub tool_timestamps: std::collections::HashMap<usize, Instant>,
```

Initialize in `Run::new` (after `last_modified: None`):

```rust
tool_timestamps: std::collections::HashMap::new(),
```

- [ ] **Step 8: Fix all compile errors from variant changes**

Run: `cargo build 2>&1`

The new fields on `SubagentEnd` and `ToolResult` will cause compile errors in:
- `src/parser.rs` — every place that constructs `SubagentEnd` or `ToolResult`
- `src/ui/transcript.rs` — every place that pattern-matches `SubagentEnd` or `ToolResult`
- `src/app.rs` — test code constructing these variants

For now, add the new fields with default values (`None` for Options, `None` for `duration_ms`) to each construction site. Use `..` struct update syntax where possible, otherwise add the fields explicitly.

In `src/parser.rs` `task_notification` handler (~line 178), update the `SubagentEnd` construction:

```rust
vec![TranscriptItem::SubagentEnd {
    summary,
    status,
    cost_usd: None,
    duration_ms: None,
    tool_uses: None,
    total_tokens: None,
}]
```

In `src/parser.rs` `parse_user` (~line 364), update `ToolResult` construction:

```rust
vec![TranscriptItem::ToolResult {
    tool_name,
    summary,
    content,
    duration_ms: None,
}]
```

In `src/ui/transcript.rs`, update the `ToolResult` match arm (~line 174) destructuring to include `duration_ms: _`:

```rust
TranscriptItem::ToolResult {
    tool_name: _,
    summary: _,
    content,
    duration_ms: _,
} => {
```

In `src/ui/transcript.rs`, update the `SubagentEnd` match arm (~line 217) destructuring to include new fields:

```rust
TranscriptItem::SubagentEnd {
    summary,
    status,
    cost_usd,
    duration_ms: _,
    tool_uses: _,
    total_tokens: _,
} => {
```

In `src/app.rs` test `stream_block_done_replaces_partial` — if it constructs any of these variants, add the new fields.

- [ ] **Step 9: Run all tests**

Run: `cargo test`
Expected: All tests pass including the new `run_stats_merge_new_fields`.

- [ ] **Step 10: Commit**

```bash
git add src/run.rs src/parser.rs src/ui/transcript.rs src/app.rs
git commit -m "feat: add data model for thinking blocks, run results, and richer stats"
```

---

### Task 2: Parser Changes

**Files:**
- Modify: `src/parser.rs`

**Depends on:** Task 1

- [ ] **Step 1: Write test for thinking block parsing**

Add to `mod tests` in `src/parser.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test parse_assistant_thinking_block`
Expected: FAIL — thinking blocks are skipped, only 1 item returned.

- [ ] **Step 3: Implement thinking block parsing**

In `src/parser.rs` `parse_assistant` function, replace the `"thinking"` match arm (~line 251-253):

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test parse_assistant_thinking_block`
Expected: PASS

- [ ] **Step 5: Write test for RunResult emission from result event**

```rust
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
```

- [ ] **Step 6: Run test to verify it fails**

Run: `cargo test parse_result_emits_run_result_item`
Expected: FAIL — currently result events return `vec![]`.

- [ ] **Step 7: Implement RunResult emission**

In `src/parser.rs`, change `parse_result_event` to return a `Vec<TranscriptItem>`:

```rust
fn parse_result_event(v: &Value, meta: &mut LineMeta) -> Vec<TranscriptItem> {
    let result = SessionResult {
        subtype: v.get("subtype").and_then(|s| s.as_str()).unwrap_or("unknown").to_string(),
        is_error: v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false),
        duration_ms: v.get("duration_ms").and_then(|n| n.as_u64()).unwrap_or(0),
        num_turns: v.get("num_turns").and_then(|n| n.as_u64()).unwrap_or(0),
        total_cost_usd: v.get("total_cost_usd").and_then(|n| n.as_f64()).unwrap_or(0.0),
        stop_reason: v.get("stop_reason").and_then(|s| s.as_str()).map(String::from),
        result_text: v.get("result").and_then(|s| s.as_str()).map(String::from),
    };

    // Parse token count from usage object
    if let Some(usage) = v.get("usage") {
        let input = usage.get("input_tokens").and_then(|n| n.as_u64()).unwrap_or(0);
        let output = usage.get("output_tokens").and_then(|n| n.as_u64()).unwrap_or(0);
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
```

Update the call site in `parse_line` (~line 59-62):

```rust
"result" => {
    let items = parse_result_event(&v, &mut meta);
    ParseResult::Parsed(items, meta)
}
```

- [ ] **Step 8: Write test for subagent usage parsing**

```rust
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
```

- [ ] **Step 9: Implement subagent usage parsing**

In `src/parser.rs` `parse_system`, update the `"task_notification"` handler to parse usage fields:

```rust
"task_notification" => {
    let summary = v.get("summary").and_then(|s| s.as_str()).unwrap_or("").to_string();
    let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string();
    let usage = v.get("usage");
    let duration_ms = usage.and_then(|u| u.get("duration_ms")).and_then(|d| d.as_u64());
    let tool_uses = usage.and_then(|u| u.get("tool_uses")).and_then(|t| t.as_u64());
    let total_tokens = usage.and_then(|u| u.get("total_tokens")).and_then(|t| t.as_u64());
    vec![TranscriptItem::SubagentEnd {
        summary,
        status,
        cost_usd: None,
        duration_ms,
        tool_uses,
        total_tokens,
    }]
}
```

- [ ] **Step 10: Run all tests**

Run: `cargo test`
Expected: All pass. The existing `parse_result_success` test may need updating since `items` is now non-empty. Update that test's assertion from `assert!(items.is_empty())` to `assert_eq!(items.len(), 1)`.

- [ ] **Step 11: Commit**

```bash
git add src/parser.rs
git commit -m "feat: parse thinking blocks, run results, and subagent usage stats"
```

---

### Task 3: Global Expand/Collapse

**Files:**
- Modify: `src/app.rs`

**Depends on:** Task 1

- [ ] **Step 1: Write test for `e` keybinding toggle**

Add to `mod tests` in `src/app.rs`:

```rust
#[test]
fn e_toggles_expanded() {
    let mut app = App::new();
    assert!(!app.expanded);
    app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
    assert!(app.expanded);
    app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
    assert!(!app.expanded);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test e_toggles_expanded`
Expected: Compile error — `expanded` field doesn't exist.

- [ ] **Step 3: Replace expanded_tools with expanded bool**

In `src/app.rs`:

1. Remove `use std::collections::HashSet;` import (line 6).
2. In `App` struct, replace `pub expanded_tools: HashSet<usize>` with `pub expanded: bool`.
3. In `App::new()`, replace `expanded_tools: HashSet::new()` with `expanded: false`.
4. Remove the `toggle_tool_expansion` method entirely (lines 331-344).
5. Remove all `self.expanded_tools.clear()` calls:
   - In `RunDiscovered` handler (~line 49)
   - In `select_next_run` (~line 299)
   - In `select_prev_run` (~line 309)
   - In `jump_to_active_run` — remove the two `.clear()` calls if present (check `handle_key` `'g'` and `'G'` handlers in RunList arm, ~lines 224 and 229)
6. In `handle_key` MainViewer arm, replace `KeyCode::Enter => self.toggle_tool_expansion()` with `KeyCode::Char('e') => { self.expanded = !self.expanded; }`.
7. Remove the now-unused `KeyCode::Enter` arm from MainViewer.

- [ ] **Step 4: Update recompute_search for new variants**

In `recompute_search` (~line 346-386), add match arms for the new `TranscriptItem` variants inside the `filter_map` closure, before the `_ => false` arm:

```rust
TranscriptItem::Thinking { text } => {
    text.to_lowercase().contains(&query_lower)
}
TranscriptItem::RunResult { stop_reason, result_text, .. } => {
    stop_reason.as_deref().unwrap_or("").to_lowercase().contains(&query_lower)
        || result_text.as_deref().unwrap_or("").to_lowercase().contains(&query_lower)
}
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All pass. The `stream_block_done_replaces_partial` test should still pass unchanged.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs
git commit -m "feat: replace per-item expand with global toggle on 'e' key"
```

---

### Task 4: Run Summary Pane

**Files:**
- Create: `src/ui/run_summary.rs`
- Modify: `src/ui/mod.rs`

**Depends on:** Task 1

- [ ] **Step 1: Create run_summary.rs with render function**

Create `src/ui/run_summary.rs`:

```rust
use crate::app::App;
use crate::run::RunStatus;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render_run_summary(frame: &mut Frame, area: Rect, app: &App) {
    let border_style = Style::default().add_modifier(Modifier::DIM);
    let block = Block::default()
        .title(" Run ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(run) = app.selected_run() else {
        return;
    };

    let dim = Style::default().add_modifier(Modifier::DIM);
    let mut lines: Vec<Line> = Vec::new();

    // Turns
    let turns = run
        .result
        .as_ref()
        .map(|r| r.num_turns)
        .unwrap_or(run.stats.num_turns);
    if turns > 0 {
        lines.push(Line::from(Span::styled(format!(" {turns} turns"), dim)));
    }

    // Tools
    if run.stats.tool_calls > 0 {
        lines.push(Line::from(Span::styled(
            format!(" {} tools", run.stats.tool_calls),
            dim,
        )));
    }

    // Agents
    if run.stats.subagent_spawns > 0 {
        lines.push(Line::from(Span::styled(
            format!(" {} agents", run.stats.subagent_spawns),
            dim,
        )));
    }

    // Tokens
    let tokens = run.stats.token_count;
    if tokens > 0 {
        let display = if tokens >= 1_000_000 {
            format!(" {:.1}M tok", tokens as f64 / 1_000_000.0)
        } else if tokens >= 1_000 {
            format!(" {}k tok", tokens / 1_000)
        } else {
            format!(" {tokens} tok")
        };
        lines.push(Line::from(Span::styled(display, dim)));
    }

    // Cost
    if let Some(cost) = run.stats.cost_usd {
        if cost > 0.0 {
            lines.push(Line::from(Span::styled(format!(" ${cost:.2}"), dim)));
        }
    }

    // Duration
    if let Some(d) = run.duration() {
        let total_secs = d.num_seconds();
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        let display = if mins > 0 {
            format!(" {mins}m {secs:02}s")
        } else {
            format!(" {secs}s")
        };
        lines.push(Line::from(Span::styled(display, dim)));
    }

    // Status line
    let (icon, icon_style, label) = match run.status {
        RunStatus::Running => ("●", Style::default().fg(Color::Green), "running".to_string()),
        RunStatus::Completed => {
            let reason = run
                .result
                .as_ref()
                .and_then(|r| r.stop_reason.as_deref())
                .unwrap_or("ok");
            ("✓", Style::default().fg(Color::Green), reason.to_string())
        }
        RunStatus::Failed => {
            let reason = run
                .result
                .as_ref()
                .and_then(|r| r.stop_reason.as_deref())
                .unwrap_or("error");
            ("✗", Style::default().fg(Color::Red), reason.to_string())
        }
        RunStatus::Unknown => ("?", dim, "unknown".to_string()),
    };
    lines.push(Line::from(vec![
        Span::styled(format!(" {icon} "), icon_style),
        Span::styled(label, dim),
    ]));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
```

- [ ] **Step 2: Wire into layout in mod.rs**

In `src/ui/mod.rs`, add `pub mod run_summary;` after the other module declarations (line 1-5).

In `render_app`, update the sidebar section. Replace the single `run_list::render_run_list(frame, horizontal[0], ...)` call with a vertical split:

```rust
if show_sidebar {
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(1)])
        .split(vertical[0]);

    // Split sidebar: run list (top) + run summary (bottom, 8 rows)
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(8)])
        .split(horizontal[0]);

    run_list::render_run_list(frame, sidebar[0], app, app.focus == FocusPane::RunList);
    run_summary::render_run_summary(frame, sidebar[1], app);
    transcript::render_transcript(
        frame,
        horizontal[1],
        app,
        app.focus == FocusPane::MainViewer,
    );
} else {
    transcript::render_transcript(frame, vertical[0], app, true);
}
```

- [ ] **Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add src/ui/run_summary.rs src/ui/mod.rs
git commit -m "feat: add run summary pane below run list in sidebar"
```

---

### Task 5: Transcript Rendering Updates

**Files:**
- Modify: `src/ui/transcript.rs`

**Depends on:** Tasks 1, 2, 3

This task updates the transcript renderer for: Thinking blocks (collapsed/expanded), RunResult display, tool timing inline, SubagentEnd richer stats, and global expand/collapse.

- [ ] **Step 1: Add Thinking block rendering**

In `src/ui/transcript.rs` `render_transcript`, add a new match arm in the `for (i, item)` loop (after `AssistantText` and before `ToolUse`):

```rust
TranscriptItem::Thinking { text } => {
    let char_count = text.len();
    if app.expanded {
        lines.push(Line::from(vec![
            Span::styled("THINK    ", label_bold(Color::DarkGray)),
            Span::styled(format!("▼ {char_count} chars"), dim),
        ]));
        for think_line in text.lines().take(20) {
            let wrapped = soft_wrap(think_line, content_cols.saturating_sub(2));
            for chunk in &wrapped {
                lines.push(Line::from(vec![
                    Span::raw("         "),
                    Span::styled(format!("│ {chunk}"), dim),
                ]));
            }
        }
    } else {
        lines.push(Line::from(vec![
            Span::styled("THINK    ", label_bold(Color::DarkGray)),
            Span::styled(format!("▶ {char_count} chars"), dim),
        ]));
    }
    lines.push(Line::default());
}
```

- [ ] **Step 2: Add RunResult rendering**

Add a match arm for `RunResult` (after `SystemEvent`):

```rust
TranscriptItem::RunResult {
    is_error,
    stop_reason,
    num_turns,
    total_cost_usd,
    duration_ms,
    result_text,
} => {
    let (icon, color) = if *is_error {
        ("✗", Color::Red)
    } else {
        ("✓", Color::Green)
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(reason) = stop_reason {
        parts.push(reason.clone());
    }
    parts.push(format!("{num_turns} turns"));
    if *total_cost_usd > 0.0 {
        parts.push(format!("${total_cost_usd:.2}"));
    }
    let total_secs = *duration_ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if mins > 0 {
        parts.push(format!("{mins}m {secs:02}s"));
    } else if total_secs > 0 {
        parts.push(format!("{secs}s"));
    }
    lines.push(Line::from(vec![
        Span::styled("RESULT   ", label_bold(color)),
        Span::styled(format!("{icon} {}", parts.join(" · ")), Style::default().fg(color)),
    ]));
    if app.expanded {
        if let Some(text) = result_text {
            for result_line in text.lines().take(20) {
                let wrapped = soft_wrap(result_line, content_cols.saturating_sub(2));
                for chunk in &wrapped {
                    lines.push(Line::from(vec![
                        Span::raw("         "),
                        Span::styled(format!("│ {chunk}"), dim),
                    ]));
                }
            }
        }
    }
    lines.push(Line::default());
}
```

- [ ] **Step 3: Update ToolUse/ToolResult rendering for global expand and tool timing**

Update the `ToolUse` arm — replace `app.expanded_tools.contains(&i)` with `app.expanded`:

```rust
if app.expanded
    && let Some(input_val) = input
{
```

Update the `ToolResult` arm:

```rust
TranscriptItem::ToolResult {
    tool_name: _,
    summary: _,
    content,
    duration_ms,
} => {
    if app.expanded {
        if let Some(dm) = duration_ms {
            let secs = *dm as f64 / 1000.0;
            lines.push(Line::from(vec![
                Span::raw("         "),
                Span::styled(format!("· {secs:.1}s"), dim),
            ]));
        }
        if let Some(content_text) = content {
            lines.push(Line::from(vec![
                Span::raw("         "),
                Span::styled("┌─ result ─", dim),
            ]));
            for content_line in content_text.lines().take(20) {
                lines.push(Line::from(vec![
                    Span::raw("         "),
                    Span::styled(format!("│ {content_line}"), dim),
                ]));
            }
            lines.push(Line::from(vec![
                Span::raw("         "),
                Span::styled("└──────────", dim),
            ]));
        }
    }
}
```

This removes the old `parent_expanded` logic that searched backwards for the parent ToolUse.

- [ ] **Step 4: Update SubagentEnd rendering with richer stats**

Replace the current `SubagentEnd` match arm:

```rust
TranscriptItem::SubagentEnd {
    summary,
    status,
    cost_usd,
    duration_ms,
    tool_uses,
    total_tokens,
} => {
    let mut parts: Vec<String> = vec![status.clone()];
    if let Some(ms) = duration_ms {
        let total_secs = ms / 1000;
        let mins = total_secs / 60;
        if mins > 0 {
            parts.push(format!("{mins}m"));
        } else {
            parts.push(format!("{total_secs}s"));
        }
    }
    if let Some(tools) = tool_uses {
        parts.push(format!("{tools} tools"));
    }
    if let Some(tokens) = total_tokens {
        if *tokens >= 1000 {
            parts.push(format!("{}k tok", tokens / 1000));
        } else {
            parts.push(format!("{tokens} tok"));
        }
    }
    if let Some(c) = cost_usd {
        parts.push(format!("${c:.2}"));
    }
    if !summary.is_empty() {
        parts.push(summary.clone());
    }
    lines.push(Line::from(vec![
        Span::styled("  └─     ", Style::default().fg(Color::Yellow)),
        Span::styled(parts.join(" · "), dim),
    ]));
    lines.push(Line::default());
}
```

- [ ] **Step 5: Build and verify**

Run: `cargo build`
Expected: Compiles successfully.

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add src/ui/transcript.rs
git commit -m "feat: render thinking blocks, run results, tool timing, and subagent stats"
```

---

### Task 6: Tool Timing in App State

**Files:**
- Modify: `src/app.rs`

**Depends on:** Tasks 1, 3

- [ ] **Step 1: Add tool timestamp tracking in update_state**

In `src/app.rs` `update_state`, in the `RunUpdated` handler (~line 52-81), add tool timing logic. This requires two passes to avoid a borrow conflict (mutable iteration + immutable index lookup cannot coexist on the same Vec).

Add this code after `run.items.extend(new_items)` and before `run.stats.merge(&stats_delta)`. First, capture the count before extending: add `let new_items_len = new_items.len();` before `run.items.extend(new_items);`.

```rust
// Track tool timestamps for timing (two-pass to satisfy borrow checker)
let base_idx = run.items.len() - new_items_len;

// Pass 1: immutable scan — stamp ToolUse items and collect (tool_result_idx, duration) pairs
let now = std::time::Instant::now();
let mut timing_updates: Vec<(usize, u64)> = Vec::new();
for offset in 0..new_items_len {
    let idx = base_idx + offset;
    match &run.items[idx] {
        TranscriptItem::ToolUse { .. } => {
            run.tool_timestamps.insert(idx, now);
        }
        TranscriptItem::ToolResult { duration_ms: None, .. } => {
            // Find the preceding ToolUse and compute delta
            if let Some(tool_idx) = (0..idx).rev().find(|&j| {
                matches!(&run.items[j], TranscriptItem::ToolUse { .. })
            }) {
                if let Some(start) = run.tool_timestamps.remove(&tool_idx) {
                    timing_updates.push((idx, start.elapsed().as_millis() as u64));
                }
            }
        }
        _ => {}
    }
}

// Pass 2: mutable — apply collected duration values
for (idx, ms) in timing_updates {
    if let TranscriptItem::ToolResult { duration_ms, .. } = &mut run.items[idx] {
        *duration_ms = Some(ms);
    }
}
```

- [ ] **Step 2: Add num_turns and token_count propagation in RunCompleted handler**

In the `RunCompleted` handler (~line 83-93), after `run.stats.cost_usd = Some(result.total_cost_usd)`, add:

```rust
run.stats.num_turns = result.num_turns;
```

- [ ] **Step 3: Build and test**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: track tool timing and propagate run stats"
```

---

### Task 7: Help Text and Final Polish

**Files:**
- Modify: `src/ui/help.rs`

**Depends on:** Task 3

- [ ] **Step 1: Update help text**

In `src/ui/help.rs`, replace the `Enter` line (~line 73-76):

```rust
Line::from(vec![
    Span::styled("    Enter         ", Style::default().fg(Color::Cyan)),
    Span::raw("Expand/collapse tool detail"),
]),
```

with:

```rust
Line::from(vec![
    Span::styled("    e             ", Style::default().fg(Color::Cyan)),
    Span::raw("Expand/collapse details"),
]),
```

- [ ] **Step 2: Build and verify**

Run: `cargo build`
Expected: Compiles.

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/ui/help.rs
git commit -m "fix: update help text for 'e' expand/collapse keybinding"
```
