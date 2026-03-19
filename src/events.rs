use std::path::PathBuf;
use chrono::{DateTime, Utc};
use crossterm::event::KeyEvent;
use crate::run::{TranscriptItem, SessionResult, RunStats};

#[derive(Debug)]
pub enum AppEvent {
    RunDiscovered { run_id: usize, path: PathBuf },
    RunUpdated {
        run_id: usize,
        new_items: Vec<TranscriptItem>,
        raw_lines: Vec<String>,
        stats_delta: RunStats,
        session_id: Option<String>,
        started_at: Option<DateTime<Utc>>,
    },
    RunCompleted { run_id: usize, result: SessionResult },
    StreamDelta { run_id: usize, text: String },
    StreamBlockDone { run_id: usize, item: TranscriptItem },
    ParseError { run_id: usize, line_no: usize, error: String },
    Key(KeyEvent),
    Resize(u16, u16),
    Tick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Transcript,
    Tools,
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    RunList,
    MainViewer,
}
