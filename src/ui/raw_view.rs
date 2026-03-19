use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use crate::app::App;

pub fn render_raw(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Raw JSONL ")
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

    let lines: Vec<Line> = run.raw_lines.iter().enumerate().map(|(i, line)| {
        let num = format!("{:>5} ", i + 1);
        let truncated = if line.len() > 200 { &line[..200] } else { line.as_str() };
        Line::from(vec![
            Span::styled(num, Style::default().fg(Color::DarkGray)),
            Span::raw(truncated.to_string()),
        ])
    }).collect();

    let paragraph = Paragraph::new(lines)
        .scroll((app.scroll_offset as u16, 0));
    frame.render_widget(paragraph, inner);
}
