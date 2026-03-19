use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

pub fn render_help(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 20, area);
    frame.render_widget(Clear, popup);

    let text = "Press any key to close\n\n\
        q         Quit\n\
        Tab       Switch pane focus\n\
        1/2/3     Transcript / Tools / Raw\n\
        j/k       Scroll / Select\n\
        /         Search\n\
        ?         Help\n\
        f         Follow live run\n\
        Enter     Expand tool detail";

    let help = Paragraph::new(text)
        .block(Block::default().title(" Help ").borders(Borders::ALL))
        .style(Style::default().fg(Color::White));
    frame.render_widget(help, popup);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
