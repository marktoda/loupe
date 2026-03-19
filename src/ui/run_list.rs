use crate::app::App;
use crate::run::RunStatus;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub fn render_run_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .runs
        .iter()
        .map(|run| {
            let (icon, icon_style) = match run.status {
                RunStatus::Running => ("●", Style::default().fg(Color::Green)),
                RunStatus::Completed => ("✓", Style::default().fg(Color::Green)),
                RunStatus::Failed => ("✗", Style::default().fg(Color::Red)),
                RunStatus::Unknown => ("?", Style::default().fg(Color::DarkGray)),
            };

            let name = run
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("???");

            let duration = run
                .duration()
                .map(|d| {
                    let mins = d.num_minutes();
                    if mins > 0 {
                        format!("{mins}m")
                    } else {
                        format!("{}s", d.num_seconds())
                    }
                })
                .unwrap_or_default();

            let status_label = match run.status {
                RunStatus::Running => "running",
                RunStatus::Completed => "ok",
                RunStatus::Failed => "failed",
                RunStatus::Unknown => "",
            };

            let line = Line::from(vec![
                Span::styled(format!(" {icon} "), icon_style),
                Span::raw(name.to_string()),
            ]);
            let detail = Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    format!("{duration} {status_label}"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            ListItem::new(vec![line, detail])
        })
        .collect();

    let mut state = ListState::default();
    state.select(app.selected_run);

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Runs ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(Style::default().bg(Color::Rgb(30, 30, 50)));

    frame.render_stateful_widget(list, area, &mut state);
}
