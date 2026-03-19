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

    if run.raw_lines.is_empty() {
        let p = Paragraph::new("No raw lines")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(p, inner);
        return;
    }

    // Gutter width: enough digits for the total line count
    let total = run.raw_lines.len();
    let gutter_width = total.to_string().len().max(4);
    // Available width for content: inner width minus gutter and separator
    let content_width = (inner.width as usize).saturating_sub(gutter_width + 1);

    let lines: Vec<Line> = run.raw_lines.iter().enumerate().map(|(i, line)| {
        let num = format!("{:>width$} ", i + 1, width = gutter_width);
        // Truncate to fit terminal width — no wrapping in raw view
        let truncated = if line.len() > content_width && content_width > 0 {
            &line[..content_width]
        } else {
            line.as_str()
        };
        Line::from(vec![
            Span::styled(num, Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)),
            Span::styled(truncated.to_string(), Style::default().fg(Color::White)),
        ])
    }).collect();

    let paragraph = Paragraph::new(lines)
        .scroll((app.scroll_offset as u16, 0));
    frame.render_widget(paragraph, inner);
}
