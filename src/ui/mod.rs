pub mod search;

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use crate::app::App;

pub fn render_app(frame: &mut Frame, app: &mut App) {
    let size = frame.area();
    let run_count = app.runs.len();
    let selected = app.selected_run.unwrap_or(0);
    let item_count = app.selected_run().map(|r| r.items.len()).unwrap_or(0);

    let text = format!(
        "Loupe v0.1.0 | {} runs | selected: {} | {} items | press q to quit",
        run_count, selected, item_count
    );
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White));
    frame.render_widget(paragraph, size);
}
