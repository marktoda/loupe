pub mod help;
pub mod run_list;
pub mod search;
pub mod status_bar;
pub mod transcript;

use crate::app::App;
use ratatui::prelude::*;

pub fn render_app(frame: &mut Frame, app: &mut App) {
    use crate::events::FocusPane;

    let size = frame.area();
    let show_sidebar = size.width >= 80;

    // Vertical split: main area + status bar (1 line)
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(size);

    if show_sidebar {
        // Horizontal split: sidebar (22 cols) + main viewer
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(22), Constraint::Min(1)])
            .split(vertical[0]);

        run_list::render_run_list(frame, horizontal[0], app, app.focus == FocusPane::RunList);
        transcript::render_transcript(
            frame,
            horizontal[1],
            app,
            app.focus == FocusPane::MainViewer,
        );
    } else {
        transcript::render_transcript(frame, vertical[0], app, true);
    }

    // Status bar or search bar
    if app.search.is_active {
        search::render_search_bar(frame, vertical[1], app);
    } else {
        status_bar::render_status_bar(frame, vertical[1], app);
    }

    if app.show_help {
        help::render_help(frame, size);
    }
}
