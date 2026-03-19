use crate::app::App;
use crate::run::RunStatus;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render_run_summary(frame: &mut Frame, area: Rect, app: &App) {
    let border_style = Style::default().add_modifier(Modifier::DIM);
    let block = Block::default()
        .title(" Run ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(run) = app.selected_run() else {
        return;
    };

    let dim = Style::default().add_modifier(Modifier::DIM);
    let mut lines: Vec<Line> = Vec::new();

    // Turns
    let turns = run
        .result
        .as_ref()
        .map(|r| r.num_turns)
        .unwrap_or(run.stats.num_turns);
    if turns > 0 {
        lines.push(Line::from(Span::styled(format!(" {turns} turns"), dim)));
    }

    // Tools
    if run.stats.tool_calls > 0 {
        lines.push(Line::from(Span::styled(
            format!(" {} tools", run.stats.tool_calls),
            dim,
        )));
    }

    // Agents
    if run.stats.subagent_spawns > 0 {
        lines.push(Line::from(Span::styled(
            format!(" {} agents", run.stats.subagent_spawns),
            dim,
        )));
    }

    // Tokens
    let tokens = run.stats.token_count;
    if tokens > 0 {
        let display = if tokens >= 1_000_000 {
            format!(" {:.1}M tok", tokens as f64 / 1_000_000.0)
        } else if tokens >= 1_000 {
            format!(" {}k tok", tokens / 1_000)
        } else {
            format!(" {tokens} tok")
        };
        lines.push(Line::from(Span::styled(display, dim)));
    }

    // Cost
    if let Some(cost) = run.stats.cost_usd {
        if cost > 0.0 {
            lines.push(Line::from(Span::styled(format!(" ${cost:.2}"), dim)));
        }
    }

    // Duration
    if let Some(d) = run.duration() {
        let total_secs = d.num_seconds();
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        let display = if mins > 0 {
            format!(" {mins}m {secs:02}s")
        } else {
            format!(" {secs}s")
        };
        lines.push(Line::from(Span::styled(display, dim)));
    }

    // Status line
    let (icon, icon_style, label) = match run.status {
        RunStatus::Running => ("●", Style::default().fg(Color::Green), "running".to_string()),
        RunStatus::Completed => {
            let reason = run
                .result
                .as_ref()
                .and_then(|r| r.stop_reason.as_deref())
                .unwrap_or("ok");
            ("✓", Style::default().fg(Color::Green), reason.to_string())
        }
        RunStatus::Failed => {
            let reason = run
                .result
                .as_ref()
                .and_then(|r| r.stop_reason.as_deref())
                .unwrap_or("error");
            ("✗", Style::default().fg(Color::Red), reason.to_string())
        }
        RunStatus::Unknown => ("?", dim, "unknown".to_string()),
    };
    lines.push(Line::from(vec![
        Span::styled(format!(" {icon} "), icon_style),
        Span::styled(label, dim),
    ]));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
