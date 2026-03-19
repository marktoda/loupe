use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use crate::app::App;
use crate::run::TranscriptItem;

pub fn render_tools(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Tools ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(run) = app.selected_run() else {
        let p = Paragraph::new("No run selected")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    for item in &run.items {
        match item {
            TranscriptItem::ToolUse { name, summary, .. } => {
                lines.push(Line::from(vec![
                    Span::styled(format!("{name:<8}"), Style::default().fg(Color::Red)),
                    Span::raw("  "),
                    Span::styled(summary.clone(), Style::default().fg(Color::White)),
                ]));
            }
            TranscriptItem::SubagentStart { description, .. } => {
                lines.push(Line::from(vec![
                    Span::styled("Agent   ", Style::default().fg(Color::Yellow)),
                    Span::raw("  "),
                    Span::styled(description.clone(), Style::default().fg(Color::Yellow)),
                ]));
            }
            TranscriptItem::SubagentEnd { summary, status, .. } => {
                lines.push(Line::from(vec![
                    Span::styled("  └─    ", Style::default().fg(Color::Yellow)),
                    Span::raw("  "),
                    Span::styled(format!("{status}: {summary}"), Style::default().fg(Color::DarkGray)),
                ]));
            }
            _ => {}
        }
    }

    let paragraph = Paragraph::new(lines)
        .scroll((app.scroll_offset as u16, 0));
    frame.render_widget(paragraph, inner);
}
