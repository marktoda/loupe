use crate::app::{App, ExpandMode};
use crate::events::FocusPane;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    // Build parts, only including non-empty segments
    let mut parts: Vec<String> = Vec::new();

    parts.push(format!("{} runs", app.runs.len()));

    if let Some(d) = app.selected_run().and_then(|r| r.duration()) {
        let mins = d.num_minutes();
        let secs = d.num_seconds() % 60;
        if mins > 0 {
            parts.push(format!("{mins}m{secs:02}s"));
        } else {
            parts.push(format!("{secs}s"));
        }
    }

    if let Some(c) = app.selected_run().and_then(|r| r.stats.cost_usd) {
        parts.push(format!("${c:.2}"));
    }

    match app.expand_mode {
        ExpandMode::Off => {}
        ExpandMode::Edits => parts.push("edits".into()),
        ExpandMode::All => parts.push("expand".into()),
    }

    if app.auto_follow {
        parts.push("follow".into());
    }

    let focus = match app.focus {
        FocusPane::RunList => "runs",
        FocusPane::MainViewer => "viewer",
    };

    let left = parts.join(" │ ");
    let text = format!(" {left}  [{focus}] Tab  / search  ? help  q quit");

    let bar = Paragraph::new(text).style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_widget(bar, area);
}
