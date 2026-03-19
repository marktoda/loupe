use crate::app::App;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render_raw(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_style = if focused {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };
    let block = Block::default()
        .title(" Raw JSONL ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let dim = Style::default().add_modifier(Modifier::DIM);

    let Some(run) = app.selected_run() else {
        let p = Paragraph::new("No run selected").style(dim);
        frame.render_widget(p, inner);
        return;
    };

    if run.raw_lines.is_empty() {
        let p = Paragraph::new("No raw lines")
            .style(dim)
            .alignment(Alignment::Center);
        frame.render_widget(p, inner);
        return;
    }

    let total = run.raw_lines.len();
    let gutter_width = total.to_string().len().max(4);
    let content_width = (inner.width as usize).saturating_sub(gutter_width + 1);

    let lines: Vec<Line> = run
        .raw_lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let num = format!("{:>width$} ", i + 1, width = gutter_width);
            let truncated = if line.len() > content_width && content_width > 0 {
                crate::parser::truncate_str(line, content_width)
            } else {
                line.as_str()
            };
            Line::from(vec![
                Span::styled(num, dim),
                Span::styled(truncated.to_string(), Style::default()),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines).scroll((app.scroll_offset as u16, 0));
    frame.render_widget(paragraph, inner);
}
