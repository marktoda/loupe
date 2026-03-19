#[derive(Debug, Default)]
pub struct SearchState {
    pub query: String,
    pub is_active: bool,
    pub matches: Vec<usize>,
    pub current_match: usize,
    pub highlights_visible: bool,
}

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use crate::app::App;

pub fn render_search_bar(frame: &mut Frame, area: Rect, app: &App) {
    let match_count = app.search.matches.len();
    let current = if match_count > 0 { app.search.current_match + 1 } else { 0 };
    let text = format!(" /{}{}", app.search.query,
        if match_count > 0 { format!("  {current}/{match_count}") } else { String::new() }
    );
    let bar = Paragraph::new(text)
        .style(Style::default().bg(Color::Rgb(50, 50, 0)).fg(Color::Yellow));
    frame.render_widget(bar, area);
}
