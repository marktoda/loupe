use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub fn render_help(frame: &mut Frame, area: Rect) {
    // Size the popup to fit, but cap at terminal size
    let width = 58.min(area.width.saturating_sub(4));
    let height = 28.min(area.height.saturating_sub(2));
    let popup = centered_rect(width, height, area);
    frame.render_widget(Clear, popup);

    let text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Global",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("    q / Ctrl-c    ", Style::default().fg(Color::Cyan)),
            Span::raw("Quit"),
        ]),
        Line::from(vec![
            Span::styled("    Tab           ", Style::default().fg(Color::Cyan)),
            Span::raw("Switch pane focus"),
        ]),
        Line::from(vec![
            Span::styled("    1 / 2 / 3     ", Style::default().fg(Color::Cyan)),
            Span::raw("Transcript / Tools / Raw view"),
        ]),
        Line::from(vec![
            Span::styled("    /             ", Style::default().fg(Color::Cyan)),
            Span::raw("Search"),
        ]),
        Line::from(vec![
            Span::styled("    ?             ", Style::default().fg(Color::Cyan)),
            Span::raw("Help"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Run List",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("    j/k  ↑/↓      ", Style::default().fg(Color::Cyan)),
            Span::raw("Select run"),
        ]),
        Line::from(vec![
            Span::styled("    g / G         ", Style::default().fg(Color::Cyan)),
            Span::raw("First / Last run"),
        ]),
        Line::from(vec![
            Span::styled("    f             ", Style::default().fg(Color::Cyan)),
            Span::raw("Jump to active run"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Main Viewer",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("    j/k  ↑/↓      ", Style::default().fg(Color::Cyan)),
            Span::raw("Scroll"),
        ]),
        Line::from(vec![
            Span::styled("    PgUp/PgDn     ", Style::default().fg(Color::Cyan)),
            Span::raw("Page scroll"),
        ]),
        Line::from(vec![
            Span::styled("    g / G         ", Style::default().fg(Color::Cyan)),
            Span::raw("Top / Bottom + auto-follow"),
        ]),
        Line::from(vec![
            Span::styled("    Enter         ", Style::default().fg(Color::Cyan)),
            Span::raw("Expand/collapse tool detail"),
        ]),
        Line::from(vec![
            Span::styled("    f             ", Style::default().fg(Color::Cyan)),
            Span::raw("Re-enable auto-follow"),
        ]),
        Line::from(vec![
            Span::styled("    n / N         ", Style::default().fg(Color::Cyan)),
            Span::raw("Next / Prev search match"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Search",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("    Enter         ", Style::default().fg(Color::Cyan)),
            Span::raw("Keep highlights, close"),
        ]),
        Line::from(vec![
            Span::styled("    Esc           ", Style::default().fg(Color::Cyan)),
            Span::raw("Clear highlights, close"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "         Press any key to close",
            Style::default().add_modifier(Modifier::DIM),
        )]),
    ];

    let help = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });
    frame.render_widget(help, popup);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
