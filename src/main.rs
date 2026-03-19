use clap::Parser;
use color_eyre::eyre::Result;
use crossterm::{
    event::EventStream,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::prelude::*;
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

mod app;
mod events;
mod parser;
mod run;
mod streaming;
mod ui;
mod watcher;

use app::App;
use events::{AppEvent, ViewMode};

#[derive(Parser)]
#[command(
    name = "loupe",
    version,
    about = "TUI viewer for Claude Code JSONL streams"
)]
struct Cli {
    /// Directory containing JSONL run logs
    path: PathBuf,

    /// Initial view mode
    #[arg(long, default_value = "transcript", value_parser = parse_view_mode)]
    view: ViewMode,
}

fn parse_view_mode(s: &str) -> Result<ViewMode, String> {
    match s {
        "transcript" => Ok(ViewMode::Transcript),
        "tools" => Ok(ViewMode::Tools),
        "raw" => Ok(ViewMode::Raw),
        _ => Err(format!("invalid view mode: {s}")),
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn setup_tracing() -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("loupe");
    std::fs::create_dir_all(&log_dir)?;
    let file_appender = tracing_appender::rolling::never(&log_dir, "loupe.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("LOUPE_LOG"))
        .with_writer(non_blocking)
        .init();
    Ok(guard)
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    if !cli.path.is_dir() {
        color_eyre::eyre::bail!("Not a directory: {}", cli.path.display());
    }

    let _log_guard = setup_tracing()?;
    let mut terminal = setup_terminal()?;

    let result = run_app(&mut terminal, cli).await;

    restore_terminal(&mut terminal)?;
    result
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, cli: Cli) -> Result<()> {
    let mut app = App::new(cli.view);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let cancel = CancellationToken::new();

    // Spawn watcher task
    let watcher_tx = tx.clone();
    let watcher_cancel = cancel.clone();
    let watch_dir = cli.path.clone();
    tokio::spawn(async move {
        if let Err(e) = watcher::run_watcher(watch_dir, watcher_tx, watcher_cancel).await {
            tracing::error!("Watcher error: {e}");
        }
    });

    // Spawn crossterm event stream task
    let term_tx = tx.clone();
    let term_cancel = cancel.clone();
    tokio::spawn(async move {
        let mut reader = EventStream::new();
        loop {
            tokio::select! {
                _ = term_cancel.cancelled() => break,
                event = reader.next() => {
                    match event {
                        Some(Ok(crossterm::event::Event::Key(key))) => {
                            let _ = term_tx.send(AppEvent::Key(key));
                        }
                        Some(Ok(crossterm::event::Event::Resize(w, h))) => {
                            let _ = term_tx.send(AppEvent::Resize(w, h));
                        }
                        Some(Ok(_)) => {} // mouse events etc — ignore
                        Some(Err(e)) => {
                            tracing::warn!("Crossterm event error: {e}");
                        }
                        None => break, // stream ended
                    }
                }
            }
        }
    });

    // Main render loop — 100ms tick (10fps), dirty-flag rendering
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut dirty = true;

    // Initial render
    terminal.draw(|frame| ui::render_app(frame, &mut app))?;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            Some(event) = rx.recv() => {
                match event {
                    AppEvent::Key(key) => {
                        app.handle_key(key);
                        if app.should_quit {
                            cancel.cancel();
                            break;
                        }
                        dirty = true;
                    }
                    AppEvent::Resize(_, _) => { dirty = true; }
                    AppEvent::Tick => {}
                    other => {
                        app.update_state(other);
                        dirty = true;
                    }
                }
            }
            _ = tick.tick() => {
                app.check_active_run_timeout();
                if dirty || app.dirty {
                    terminal.draw(|frame| ui::render_app(frame, &mut app))?;
                    dirty = false;
                    app.dirty = false;
                }
            }
        }
    }

    Ok(())
}
