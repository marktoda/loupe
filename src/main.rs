use clap::Parser;
use std::path::PathBuf;
use color_eyre::eyre::Result;

mod app;
mod events;
mod parser;
mod run;
mod streaming;
mod watcher;
mod ui;

use events::ViewMode;

#[derive(Parser)]
#[command(name = "loupe", version, about = "TUI viewer for Claude Code JSONL streams")]
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

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    if !cli.path.is_dir() {
        color_eyre::eyre::bail!("Not a directory: {}", cli.path.display());
    }

    // TODO: setup tracing, terminal, run app
    println!("Watching: {}", cli.path.display());
    Ok(())
}
