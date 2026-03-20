use crate::app::App;
use crate::run::TranscriptItem;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

const LABEL_WIDTH: usize = 9; // "ASSIST   " etc.

/// Split `text` into styled spans, highlighting all case-insensitive occurrences of `query`.
fn highlight_text(text: &str, query: &str, base_style: Style) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }
    let query_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in text_lower.match_indices(&query_lower as &str) {
        let end = start + query_lower.len();
        // Skip matches whose byte offsets don't land on char boundaries in the
        // original text (can happen when lowercasing changes byte length, e.g. ß→ss).
        if !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
            || !text.is_char_boundary(last_end)
        {
            continue;
        }
        if start > last_end {
            spans.push(Span::styled(text[last_end..start].to_string(), base_style));
        }
        spans.push(Span::styled(
            text[start..end].to_string(),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        last_end = end;
    }
    if last_end < text.len() && text.is_char_boundary(last_end) {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
    }
    spans
}

/// Soft-wrap a string to fit within `max_cols` columns.
/// Returns a vec of string slices, each fitting within the width.
/// Uses char boundaries (not grapheme clusters) — good enough for terminal text.
fn soft_wrap(text: &str, max_cols: usize) -> Vec<&str> {
    if max_cols == 0 || text.is_empty() {
        return vec![text];
    }
    let mut result = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        match remaining.char_indices().nth(max_cols) {
            None => {
                result.push(remaining);
                break;
            }
            Some((byte_end, _)) => {
                result.push(&remaining[..byte_end]);
                remaining = &remaining[byte_end..];
            }
        }
    }
    result
}

/// Truncate a string to fit within `max_chars` characters, appending `…` if truncated.
fn truncate(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

pub fn render_transcript(frame: &mut Frame, area: Rect, app: &mut App, focused: bool) {
    let border_style = if focused {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };
    let block = Block::default()
        .title(" Transcript ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(run) = app.selected_run() else {
        let msg = if app.runs.is_empty() {
            "Waiting for runs..."
        } else {
            "Select a run"
        };
        let p = Paragraph::new(msg)
            .style(Style::default().add_modifier(Modifier::DIM))
            .alignment(Alignment::Center);
        frame.render_widget(p, inner);
        return;
    };

    if run.items.is_empty() {
        let p = Paragraph::new("Parsing...")
            .style(Style::default().add_modifier(Modifier::DIM))
            .alignment(Alignment::Center);
        frame.render_widget(p, inner);
        return;
    }

    let search_active = app.search.highlights_visible && !app.search.query.is_empty();
    let query = app.search.query.clone();

    let label_bold = |color: Color| Style::default().fg(color).add_modifier(Modifier::BOLD);
    let dim = Style::default().add_modifier(Modifier::DIM);
    let default = Style::default();

    // Content area width for text wrapping (minus the 9-char label column)
    let content_cols = (inner.width as usize).saturating_sub(LABEL_WIDTH);

    let mut lines: Vec<Line> = Vec::new();

    for item in &run.items {
        match item {
            TranscriptItem::SessionStart { model, tools, .. } => {
                lines.push(Line::from(vec![
                    Span::styled("SESSION  ", label_bold(Color::Green)),
                    Span::styled(
                        truncate(&format!("{model} · {} tools", tools.len()), content_cols),
                        Style::default().fg(Color::Green),
                    ),
                ]));
                lines.push(Line::default());
            }
            TranscriptItem::AssistantText { text, is_partial } => {
                let display = if *is_partial {
                    format!("{text}▌")
                } else {
                    text.clone()
                };
                // Pre-wrap: split each source line to fit within content_cols
                for (li, source_line) in display.lines().enumerate() {
                    let wrapped = soft_wrap(source_line, content_cols);
                    for (wi, chunk) in wrapped.iter().enumerate() {
                        let content_spans = if search_active {
                            highlight_text(chunk, &query, default)
                        } else {
                            vec![Span::styled(chunk.to_string(), default)]
                        };
                        let prefix = if li == 0 && wi == 0 {
                            Span::styled("ASSIST   ", label_bold(Color::Cyan))
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
            TranscriptItem::ToolUse {
                name,
                summary,
                input,
            } => {
                let summary_max = content_cols.saturating_sub(name.len() + 2);
                lines.push(Line::from(vec![
                    Span::styled("TOOL     ", label_bold(Color::Magenta)),
                    Span::styled(name.to_string(), Style::default().fg(Color::Red)),
                    Span::raw("  "),
                    Span::styled(truncate(summary, summary_max), dim),
                ]));

                if app.expanded
                    && let Some(input_val) = input
                {
                    let json_str = serde_json::to_string_pretty(input_val).unwrap_or_default();
                    for json_line in json_str.lines().take(15) {
                        lines.push(Line::from(vec![
                            Span::raw("         "),
                            Span::styled(format!("│ {json_line}"), dim),
                        ]));
                    }
                }
            }
            TranscriptItem::ToolResult {
                tool_name: _,
                summary: _,
                content,
                duration_ms: _,
            } => {
                if app.expanded {
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
            TranscriptItem::SubagentStart { description, .. } => {
                lines.push(Line::from(vec![
                    Span::styled("AGENT    ", label_bold(Color::Yellow)),
                    Span::styled(
                        truncate(description, content_cols),
                        Style::default().fg(Color::Yellow),
                    ),
                ]));
            }
            TranscriptItem::SubagentProgress {
                description,
                tool_name,
            } => {
                let tool = tool_name.as_deref().unwrap_or("");
                let text = format!("{tool}  {description}");
                lines.push(Line::from(vec![
                    Span::styled("  ├─     ", Style::default().fg(Color::Yellow)),
                    Span::styled(truncate(&text, content_cols), dim),
                ]));
            }
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
            TranscriptItem::Error { message } => {
                lines.push(Line::from(vec![
                    Span::styled("ERROR    ", label_bold(Color::Red)),
                    Span::styled(message.clone(), Style::default().fg(Color::Red)),
                ]));
            }
            TranscriptItem::SystemEvent { label, detail } => {
                lines.push(Line::from(vec![
                    Span::styled("SYSTEM   ", dim),
                    Span::styled(format!("{label}: {detail}"), dim),
                ]));
            }
            TranscriptItem::Thinking { text } => {
                let char_count = text.chars().count();
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
                    Span::styled(
                        format!("{icon} {}", parts.join(" · ")),
                        Style::default().fg(color),
                    ),
                ]));
                if app.expanded {
                    if let Some(text) = result_text {
                        for result_line in text.lines().take(20) {
                            let wrapped =
                                soft_wrap(result_line, content_cols.saturating_sub(2));
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
        }
    }

    // No Wrap — we pre-wrapped above, so lines.len() IS the rendered row count.
    let total = lines.len();
    let visible = inner.height as usize;

    if app.auto_follow && total > visible {
        app.scroll_offset = total - visible;
    }
    // Clamp scroll to valid range
    let max_scroll = total.saturating_sub(visible);
    if app.scroll_offset > max_scroll {
        app.scroll_offset = max_scroll;
    }

    let paragraph =
        Paragraph::new(lines).scroll((app.scroll_offset.min(u16::MAX as usize) as u16, 0));
    frame.render_widget(paragraph, inner);
}
