use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use crate::app::App;
use crate::events::ViewMode;

pub fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let mode_str = match app.view_mode {
        ViewMode::Transcript => "transcript",
        ViewMode::Tools => "tools",
        ViewMode::Raw => "raw",
    };

    let run_count = app.runs.len();

    let duration_str = app.selected_run()
        .and_then(|r| r.duration())
        .map(|d| {
            let mins = d.num_minutes();
            let secs = d.num_seconds() % 60;
            if mins > 0 { format!("{mins}m{secs:02}s") } else { format!("{secs}s") }
        })
        .unwrap_or_default();

    let cost_str = app.selected_run()
        .and_then(|r| r.stats.cost_usd)
        .map(|c| format!("${c:.2}"))
        .unwrap_or_default();

    let follow_str = if app.auto_follow { "follow" } else { "" };

    let text = format!(
        " {mode_str} | {run_count} runs | {duration_str} | {cost_str} | {follow_str} | / search  ? help  q quit"
    );

    let bar = Paragraph::new(text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    frame.render_widget(bar, area);
}
