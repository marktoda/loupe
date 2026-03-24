use crate::app::{App, ExpandMode};
use crate::run::TranscriptItem;
use crate::ui::markdown::{self, MarkdownRenderOptions};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

const LABEL_WIDTH: usize = 9; // "ASSIST   " etc.

fn soft_wrap(text: &str, max_cols: usize) -> Vec<&str> {
    markdown::soft_wrap_plain(text, max_cols)
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
    let title = match app.expand_mode {
        ExpandMode::Off => " Transcript ",
        ExpandMode::Edits => " Transcript [edits] ",
        ExpandMode::All => " Transcript [all] ",
    };
    let block = Block::default()
        .title(title)
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

    let content_cols = (inner.width as usize).saturating_sub(LABEL_WIDTH);
    let run_id = app.selected_run;
    let item_count = run.items.len();
    let expand_mode = app.expand_mode;
    let search_visible = app.search.highlights_visible;

    // Rebuild cached lines only when content changes — not on scroll
    if !app.transcript_cache.is_valid(
        run_id,
        item_count,
        expand_mode,
        &app.search.query,
        search_visible,
        content_cols,
    ) {
        let search_active = search_visible && !app.search.query.is_empty();
        let query = app.search.query.clone();
        let label_bold =
            |color: Color| Style::default().fg(color).add_modifier(Modifier::BOLD);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let default = Style::default();

        let mut lines: Vec<Line> = Vec::new();

        // Re-borrow run inside cache rebuild scope
        let run = app.runs.get(run_id.unwrap_or(0)).unwrap();
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
                lines.extend(markdown::render_markdown_block(
                    &display,
                    &MarkdownRenderOptions {
                        label: "ASSIST   ",
                        label_style: label_bold(Color::Cyan),
                        content_cols,
                        base_style: default,
                        search_query: &query,
                        search_active,
                    },
                ));
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

                if let Some(input_val) = input {
                    let is_edit = name == "Edit";
                    let show = match app.expand_mode {
                        ExpandMode::Off => false,
                        ExpandMode::Edits => is_edit,
                        ExpandMode::All => true,
                    };
                    if show {
                        if is_edit {
                            lines.extend(super::highlight::render_edit(
                                input_val,
                                content_cols,
                            ));
                        } else {
                            let json_str =
                                serde_json::to_string_pretty(input_val).unwrap_or_default();
                            for json_line in json_str.lines().take(15) {
                                lines.push(Line::from(vec![
                                    Span::raw("         "),
                                    Span::styled(format!("│ {json_line}"), dim),
                                ]));
                            }
                        }
                    }
                }
            }
            TranscriptItem::ToolResult {
                tool_name,
                summary: _,
                content,
                duration_ms: _,
            } => {
                if let Some(content_text) = content {
                    let is_patch = tool_name == "apply_patch";
                    let show = match app.expand_mode {
                        ExpandMode::Off => false,
                        ExpandMode::Edits => is_patch,
                        ExpandMode::All => true,
                    };
                    if show {
                        if is_patch {
                            lines.extend(super::highlight::render_patch(
                                content_text,
                                content_cols,
                            ));
                        } else {
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
                if app.expand_mode == ExpandMode::All {
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
                if app.expand_mode == ExpandMode::All {
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
            TranscriptItem::UserMessage { text } => {
                lines.extend(markdown::render_markdown_block(
                    text,
                    &MarkdownRenderOptions {
                        label: "USER     ",
                        label_style: label_bold(Color::Blue),
                        content_cols,
                        base_style: default,
                        search_query: &query,
                        search_active,
                    },
                ));
            }
        }
        }

        let query_for_cache = app.search.query.clone();
        app.transcript_cache.store(
            lines,
            run_id,
            item_count,
            expand_mode,
            &query_for_cache,
            search_visible,
            content_cols,
        );
    }

    // Use cached lines for rendering — scrolling is now free
    let total = app.transcript_cache.lines.len();
    let visible = inner.height as usize;

    if app.auto_follow && total > visible {
        app.scroll_offset = total - visible;
    }
    let max_scroll = total.saturating_sub(visible);
    if app.scroll_offset > max_scroll {
        app.scroll_offset = max_scroll;
    }

    // Render visible lines directly into buffer — no cloning
    let start = app.scroll_offset;
    let buf = frame.buffer_mut();
    for (i, y) in (inner.y..inner.bottom()).enumerate() {
        let line_idx = start + i;
        if line_idx < total {
            let line = &app.transcript_cache.lines[line_idx];
            buf.set_line(inner.x, y, line, inner.width);
        }
    }
}
