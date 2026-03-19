use crate::app::App;
use crate::events::FocusPane;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let run_count = app.runs.len();

    let duration_str = app
        .selected_run()
        .and_then(|r| r.duration())
        .map(|d| {
            let mins = d.num_minutes();
            let secs = d.num_seconds() % 60;
            if mins > 0 {
                format!("{mins}m{secs:02}s")
            } else {
                format!("{secs}s")
            }
        })
        .unwrap_or_default();

    let cost_str = app
        .selected_run()
        .and_then(|r| r.stats.cost_usd)
        .map(|c| format!("${c:.2}"))
        .unwrap_or_default();

    let follow_str = if app.auto_follow { "follow" } else { "" };

    let focus_str = match app.focus {
        FocusPane::RunList => "runs",
        FocusPane::MainViewer => "viewer",
    };

    let text = format!(
        " {run_count} runs | {duration_str} | {cost_str} | {follow_str} | [{focus_str}] Tab | / search  ? help  q quit"
    );

    // Reversed style = terminal's bg becomes fg and vice versa. Always readable.
    let bar = Paragraph::new(text).style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_widget(bar, area);
}
