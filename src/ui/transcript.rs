use crate::app::App;
use crate::run::TranscriptItem;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// Split `text` into styled spans, highlighting all case-insensitive occurrences of `query`
/// with a yellow background. Returns a single span if query is empty.
fn highlight_text(text: &str, query: &str, base_style: Style) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }
    let query_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in text_lower.match_indices(&query_lower as &str) {
        let end = start + query.len();
        if start > last_end {
            spans.push(Span::styled(text[last_end..start].to_string(), base_style));
        }
        spans.push(Span::styled(
            text[start..end].to_string(),
            Style::default().bg(Color::Rgb(80, 80, 0)).fg(Color::Yellow),
        ));
        last_end = end;
    }
    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
    }
    spans
}

pub fn render_transcript(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .title(" Transcript ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(run) = app.selected_run() else {
        let msg = if app.runs.is_empty() {
            "Waiting for runs..."
        } else {
            "Select a run"
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(p, inner);
        return;
    };

    if run.items.is_empty() {
        let p = Paragraph::new("Parsing...")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(p, inner);
        return;
    }

    let search_active = app.search.highlights_visible && !app.search.query.is_empty();
    let query = app.search.query.clone();

    let mut lines: Vec<Line> = Vec::new();

    for (i, item) in run.items.iter().enumerate() {
        match item {
            TranscriptItem::SessionStart { model, tools, .. } => {
                lines.push(Line::from(vec![
                    Span::styled(
                        "SESSION  ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{model} · {} tools", tools.len()),
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
                let text_lines: Vec<&str> = display.lines().collect();
                let base_style = Style::default().fg(Color::White);
                for (li, line_text) in text_lines.iter().enumerate() {
                    let content_spans = if search_active {
                        highlight_text(line_text, &query, base_style)
                    } else {
                        vec![Span::styled(line_text.to_string(), base_style)]
                    };
                    if li == 0 {
                        let mut spans = vec![Span::styled(
                            "ASSIST   ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        )];
                        spans.extend(content_spans);
                        lines.push(Line::from(spans));
                    } else {
                        let mut spans = vec![Span::raw("         ")];
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
                lines.push(Line::from(vec![
                    Span::styled(
                        "TOOL     ",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(name.to_string(), Style::default().fg(Color::Red)),
                    Span::raw("  "),
                    Span::styled(summary.clone(), Style::default().fg(Color::DarkGray)),
                ]));

                if app.expanded_tools.contains(&i)
                    && let Some(input_val) = input
                {
                    let json_str = serde_json::to_string_pretty(input_val).unwrap_or_default();
                    for json_line in json_str.lines().take(15) {
                        lines.push(Line::from(vec![
                            Span::raw("         "),
                            Span::styled(
                                format!("│ {json_line}"),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }
                }
            }
            TranscriptItem::ToolResult {
                tool_name: _,
                summary: _,
                content,
            } => {
                let parent_expanded = (0..i)
                    .rev()
                    .find(|&j| matches!(&run.items[j], TranscriptItem::ToolUse { .. }))
                    .is_some_and(|j| app.expanded_tools.contains(&j));

                if parent_expanded && let Some(content_text) = content {
                    lines.push(Line::from(vec![
                        Span::raw("         "),
                        Span::styled("┌─ result ─", Style::default().fg(Color::DarkGray)),
                    ]));
                    for content_line in content_text.lines().take(20) {
                        lines.push(Line::from(vec![
                            Span::raw("         "),
                            Span::styled(
                                format!("│ {content_line}"),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }
                    lines.push(Line::from(vec![
                        Span::raw("         "),
                        Span::styled("└──────────", Style::default().fg(Color::DarkGray)),
                    ]));
                }
            }
            TranscriptItem::SubagentStart { description, .. } => {
                lines.push(Line::from(vec![
                    Span::styled(
                        "AGENT    ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(description.clone(), Style::default().fg(Color::Yellow)),
                ]));
            }
            TranscriptItem::SubagentProgress {
                description,
                tool_name,
            } => {
                let tool = tool_name.as_deref().unwrap_or("");
                lines.push(Line::from(vec![
                    Span::styled("  ├─     ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("{tool}  {description}"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
            TranscriptItem::SubagentEnd {
                summary,
                status,
                cost_usd,
            } => {
                let cost = cost_usd.map(|c| format!("${c:.2}")).unwrap_or_default();
                lines.push(Line::from(vec![
                    Span::styled("  └─     ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("{status} · {summary} · {cost}"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                lines.push(Line::default());
            }
            TranscriptItem::Error { message } => {
                lines.push(Line::from(vec![
                    Span::styled(
                        "ERROR    ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(message.clone(), Style::default().fg(Color::Red)),
                ]));
            }
            TranscriptItem::SystemEvent { label, detail } => {
                lines.push(Line::from(vec![
                    Span::styled("SYSTEM   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{label}: {detail}"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }
    }

    let total_lines = lines.len() as u16;
    let visible_height = inner.height;
    if app.auto_follow && total_lines > visible_height {
        app.scroll_offset = (total_lines - visible_height) as usize;
    }

    let paragraph = Paragraph::new(lines)
        .scroll((app.scroll_offset as u16, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, inner);
}
