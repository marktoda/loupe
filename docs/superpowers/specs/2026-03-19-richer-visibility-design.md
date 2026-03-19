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
- Only non-empty stats are shown (same pattern as status bar)

### Layout

The left sidebar keeps its 22-column width. The run list occupies `sidebar_height - 8` rows. The summary pane gets the bottom 8 rows (including its border). When the terminal is very short, the summary pane takes priority up to a minimum run list height of 3 rows.

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
- Searchable (included in search matching against the text content)
- Style: dimmed (thinking is secondary to the assistant's visible output)

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

Approximate duration displayed after the tool summary, dimmed. Computed by tracking wall-clock time: record `Instant::now()` when a `ToolUse` item is added, and compute the delta when the corresponding `ToolResult` arrives.

Implementation: add `created_at: Option<Instant>` to `ToolUse` and `ToolResult` variants isn't viable (Instant isn't serializable and pollutes the data model). Instead, track timing in a separate `HashMap<usize, Instant>` in `App` keyed by item index. When rendering a `ToolUse`, look up elapsed time if a subsequent `ToolResult` exists.

Simpler approach: when the watcher sends `RunUpdated` with new items, stamp each `ToolUse` with the current time in a side map (`tool_timestamps: HashMap<usize, Instant>` on `Run`). When a `ToolResult` arrives, compute delta from the preceding `ToolUse` timestamp. Store the duration on the `ToolResult` item itself as `duration_ms: Option<u64>`.

If timing can't be computed (e.g., no preceding ToolUse, or events arrive in same batch), omit the duration — no placeholder.

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

Render non-None fields inline, dimmed, separated by ` · `.

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

Emitted when a `"result"` event is parsed. `result_text` is available for expansion via the global toggle (shows the final result summary from Claude).

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

## 5. Data Model Changes Summary

### New TranscriptItem variants

- `Thinking { text: String }`
- `RunResult { is_error: bool, stop_reason: Option<String>, num_turns: u64, total_cost_usd: f64, duration_ms: u64, result_text: Option<String> }`

### Modified TranscriptItem variants

- `SubagentEnd`: add `duration_ms: Option<u64>`, `tool_uses: Option<u64>`, `total_tokens: Option<u64>`

### RunStats additions

- `num_turns: u64` — incremented from result event
- `token_count: u64` — accumulated from result event's usage or task_notification usage

### App state changes

- Remove `expanded_tools: HashSet<usize>`
- Add `expanded: bool` (default false)
- Add `tool_timestamps: HashMap<usize, Instant>` on `Run` for tool timing

### Parser changes

- `parse_assistant`: emit `Thinking` items instead of skipping thinking blocks
- `parse_system` (`task_notification`): parse `duration_ms`, `tool_uses`, `total_tokens` from usage object
- `parse_result_event`: emit `RunResult` transcript item in addition to setting `meta.session_result`

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
