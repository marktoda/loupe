# Richer Visibility

Add a run summary pane, thinking block display, inline timing/stats, and a global expand/collapse toggle.

## 1. Run Summary Pane

Fixed 8-row pane at the bottom of the left sidebar, below the run list.

### Content

Key-value pairs, one per line, left-aligned:

```
├ Run ───────────────┤
│ 12 turns           │
│ 47 tools           │
│  3 agents          │
│ 128k tok           │
│ $4.82              │
│ 21m 14s            │
│ ● running          │
└────────────────────┘
```

- **Turns**: from `SessionResult.num_turns` (completed) or accumulated count (ongoing)
- **Tools**: from `RunStats.tool_calls`
- **Agents**: from `RunStats.subagent_spawns`
- **Tokens**: from `RunStats.token_count` (new field)
- **Cost**: from `RunStats.cost_usd`
- **Duration**: `started_at` to now (running) or to `ended_at` (completed)
- **Status**: icon + label. For completed/failed runs, append stop reason (e.g., `✓ end_turn`, `✗ max_tokens`)

### Behavior

- Reflects whichever run is selected in the run list
- Updates live as events arrive
- If no run selected: dimmed placeholder
- Only non-empty stats are shown: `Option` fields hidden when `None`, integer fields hidden when `0` (e.g., a run with 0 subagents hides the agents line)

### Layout

The left sidebar keeps its 22-column width. Split `horizontal[0]` vertically in `src/ui/mod.rs` using `Layout::vertical([Constraint::Min(3), Constraint::Length(8)])`. The run list gets the top chunk (minimum 3 rows), the summary pane gets the bottom 8 rows (including its border). When the terminal is very short, `Min(3)` ensures the run list never disappears entirely; the summary pane naturally shrinks if there isn't room for both.

## 2. Thinking Blocks

New transcript item for Claude's thinking/reasoning content.

### Display

```
THINK    ▶ 847 chars                     (collapsed, default)

THINK    ▼ 847 chars                     (expanded)
         │ The user wants me to look at
         │ the routing table and check
         │ for capacity constraints...
```

### Behavior

- Collapsed by default, controlled by the global expand/collapse toggle (see section 4)
- Collapsed: shows `▶` and character count
- Expanded: shows `▼`, character count, then content lines prefixed with `│`, truncated at 20 lines, soft-wrapped to content width
- Searchable: `recompute_search` in `app.rs` must add a match arm for `TranscriptItem::Thinking` (and `TranscriptItem::RunResult`) — the existing `_ => false` catch-all would silently exclude them
- Style: dimmed (thinking is secondary to the assistant's visible output)
- Note: thinking content only appears after the full `assistant` event is written to the JSONL file — there is no live streaming preview of thinking blocks (the `DeltaAccumulator` skips `thinking_delta` events). This is acceptable; live streaming is for the visible assistant text.

### Data Model

New variant:

```rust
TranscriptItem::Thinking { text: String }
```

### Parser Change

In `parse_assistant`, the `"thinking"` block type currently skips. Change to:

```rust
"thinking" => {
    let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
    if !text.is_empty() {
        items.push(TranscriptItem::Thinking { text });
    }
}
```

## 3. Inline Timing & Stats

### Tool Duration

```
TOOL     Read  src/routing/ssp/mod.rs  · 0.4s
```

Approximate duration displayed after the tool summary, dimmed. Add `duration_ms: Option<u64>` to the `ToolResult` variant.

**Live runs:** Track wall-clock time in a side map (`tool_timestamps: HashMap<usize, Instant>` on `Run`). When the watcher sends `RunUpdated` with new items, stamp each `ToolUse` with `Instant::now()` keyed by its item index. When a `ToolResult` arrives in a subsequent batch, compute the delta from the preceding `ToolUse` timestamp and store it on the `ToolResult`.

**Limitation:** Tool timing is only available for live runs where the `ToolUse` and `ToolResult` arrive in separate watcher batches (separated by at least the 200ms coalesce window). For the initial file parse of completed runs, all items arrive in a single batch — timing will be unavailable and omitted. For fast-completing tools in live runs, the same-batch case applies. This is an acceptable trade-off: timing is most useful for slow, long-running tools (Bash commands, large reads) which naturally span multiple batches.

If timing can't be computed (no preceding ToolUse, or same-batch arrival), omit the duration — no placeholder.

### Subagent End Stats

```
  └─     completed · 4m · 9 tools · 128k tok · $1.23
```

The `task_notification` event has a `usage` object with `duration_ms`, `tool_uses`, and `total_tokens`. Parse these into `SubagentEnd`:

```rust
TranscriptItem::SubagentEnd {
    summary: String,
    status: String,
    cost_usd: Option<f64>,
    duration_ms: Option<u64>,    // new
    tool_uses: Option<u64>,      // new
    total_tokens: Option<u64>,   // new
}
```

Render non-None fields inline, dimmed, separated by ` · `. Build a `Vec<String>` of present segments, then join with ` · ` — avoids trailing separator bugs. Apply this same pattern to the existing `SubagentEnd` rendering which currently has a trailing ` · ` when `cost_usd` is `None`.

### Run Result in Transcript

New transcript item rendered at the end of a completed run:

```
RESULT   ✓ end_turn · 12 turns · $4.82 · 21m 14s
```

Or for failures:

```
RESULT   ✗ max_tokens · 3 turns · $0.41 · 2m 05s
```

New variant:

```rust
TranscriptItem::RunResult {
    is_error: bool,
    stop_reason: Option<String>,
    num_turns: u64,
    total_cost_usd: f64,
    duration_ms: u64,
    result_text: Option<String>,
}
```

Emitted when a `"result"` event is parsed. The `parse_result_event` function must return the `RunResult` item, and its call site in `parse_line` must change from `ParseResult::Parsed(vec![], meta)` to include the returned item. `result_text` is available for expansion via the global toggle (shows the final result summary from Claude).

## 4. Global Expand/Collapse

Replace the current per-item `expanded_tools: HashSet<usize>` with a single boolean toggle.

### Keybinding

`e` in MainViewer focus: toggles `app.expanded` between `true` and `false`.

### What it controls

- Tool input JSON (currently behind per-item toggle)
- Tool result content blocks
- Thinking block content
- RunResult result_text

### State

```rust
// Remove:
pub expanded_tools: HashSet<usize>,

// Add:
pub expanded: bool,  // default: false
```

### Rendering

All expansion checks change from `app.expanded_tools.contains(&i)` to `app.expanded`.

The `toggle_tool_expansion` method is removed. The `Enter` key in MainViewer is freed up (or can be repurposed later).

All `expanded_tools.clear()` call sites in `app.rs` (in `RunDiscovered`, `select_next_run`, `select_prev_run`, and `jump_to_active_run` handlers) are removed — the global toggle persists across run switches.

## 5. Data Model Changes Summary

### New TranscriptItem variants

- `Thinking { text: String }`
- `RunResult { is_error: bool, stop_reason: Option<String>, num_turns: u64, total_cost_usd: f64, duration_ms: u64, result_text: Option<String> }`

### Modified TranscriptItem variants

- `SubagentEnd`: add `duration_ms: Option<u64>`, `tool_uses: Option<u64>`, `total_tokens: Option<u64>`

### RunStats additions

- `num_turns: u64` — set from `SessionResult.num_turns` when a `result` event arrives (via `RunCompleted` handler in `app.rs`). `RunStats::merge` must be updated to handle this field.
- `token_count: u64` — set from `result` event's top-level `usage.input_tokens + usage.output_tokens` when available. For ongoing runs before a `result` event, this will be 0 (token counts aren't available per-message in the JSONL format). Subagent token counts from `task_notification` are NOT added separately — the outer run's `result` event includes the total across all subagents, so adding both would double-count.

### App state changes

- Remove `expanded_tools: HashSet<usize>`
- Add `expanded: bool` (default false)
- Add `tool_timestamps: HashMap<usize, Instant>` on `Run` for tool timing

### Parser changes

- `parse_assistant`: emit `Thinking` items instead of skipping thinking blocks
- `parse_system` (`task_notification`): parse `duration_ms`, `tool_uses`, `total_tokens` from usage object
- `parse_result_event`: return `RunResult` transcript item in addition to setting `meta.session_result`; call site in `parse_line` must include the returned item instead of `vec![]`
- `parse_result_event`: parse `usage.input_tokens` and `usage.output_tokens` into `meta.stats_delta.token_count`

## 6. Files to Modify

- `src/run.rs` — new TranscriptItem variants, RunStats fields, tool_timestamps on Run
- `src/parser.rs` — thinking blocks, subagent usage parsing, RunResult emission
- `src/events.rs` — no changes needed (existing events sufficient)
- `src/app.rs` — replace expanded_tools with expanded bool, `e` keybinding, tool timestamp tracking
- `src/ui/transcript.rs` — render Thinking, RunResult, inline tool timing, subagent stats, global expand
- `src/ui/mod.rs` — layout split for summary pane
- `src/ui/run_list.rs` — may need height adjustment for new layout
- New: `src/ui/run_summary.rs` — render the run summary pane
- `src/streaming.rs` — no changes needed
- `src/ui/help.rs` — update help text for `e` keybinding
- `src/ui/status_bar.rs` — possibly remove duplicate stats now shown in summary pane
