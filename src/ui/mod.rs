pub mod help;
pub mod run_list;
pub mod run_summary;
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
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(22), Constraint::Min(1)])
            .split(vertical[0]);

        // Split sidebar: run list (top) + run summary (bottom, 8 rows)
        let sidebar = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(8)])
            .split(horizontal[0]);

        run_list::render_run_list(frame, sidebar[0], app, app.focus == FocusPane::RunList);
        run_summary::render_run_summary(frame, sidebar[1], app);
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
