use crate::app::App;
use crate::run::TranscriptItem;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render_tools(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused { Color::Blue } else { Color::Gray };
    let title = if focused { " Tools (Tab) " } else { " Tools " };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(run) = app.selected_run() else {
        let p = Paragraph::new("No run selected").style(Style::default().fg(Color::Gray));
        frame.render_widget(p, inner);
        return;
    };

    let mut tool_lines: Vec<Line> = Vec::new();
    for item in &run.items {
        match item {
            TranscriptItem::ToolUse { name, summary, .. } => {
                let name_col = format!("{name:<16}");
                tool_lines.push(Line::from(vec![
                    Span::styled(name_col, Style::default().fg(Color::Magenta)),
                    Span::styled(summary.clone(), Style::default().fg(Color::White)),
                ]));
            }
            TranscriptItem::ToolResult {
                tool_name, summary, ..
            } => {
                let name_col = format!("{:<16}", format!("  ↳ {tool_name}"));
                tool_lines.push(Line::from(vec![
                    Span::styled(name_col, Style::default().fg(Color::Gray)),
                    Span::styled(summary.clone(), Style::default().fg(Color::Gray)),
                ]));
            }
            TranscriptItem::SubagentStart { description, .. } => {
                tool_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:<16}", "AGENT"),
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
                let tool = tool_name.as_deref().unwrap_or("…");
                let prefix = format!("{:<16}", format!("  ├─ {tool}"));
                tool_lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Yellow)),
                    Span::styled(description.clone(), Style::default().fg(Color::Gray)),
                ]));
            }
            TranscriptItem::SubagentEnd {
                summary,
                status,
                cost_usd,
            } => {
                let cost = cost_usd.map(|c| format!("  ${c:.2}")).unwrap_or_default();
                tool_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:<16}", "  └─"),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(
                        format!("{status}: {summary}{cost}"),
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            }
            _ => {}
        }
    }

    if tool_lines.is_empty() {
        let p = Paragraph::new("No tool calls in this run")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        frame.render_widget(p, inner);
        return;
    }

    let paragraph = Paragraph::new(tool_lines).scroll((app.scroll_offset as u16, 0));
    frame.render_widget(paragraph, inner);
}
