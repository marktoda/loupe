use std::collections::HashMap;
use std::path::PathBuf;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::events::AppEvent;
use crate::run::{RunStats, TranscriptItem};
use crate::streaming::DeltaAccumulator;
use crate::{parser, streaming};

struct WatchedFile {
    run_id: usize,
    bytes_read: u64,
    has_result: bool,
}

fn discover_files(dir: &PathBuf) -> color_eyre::Result<Vec<PathBuf>> {
    let mut files: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .collect();
    files.sort_by_key(|p| {
        p.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    Ok(files)
}

fn parse_file_initial(
    run_id: usize,
    path: &PathBuf,
    tx: &UnboundedSender<AppEvent>,
) -> color_eyre::Result<WatchedFile> {
    let content = std::fs::read_to_string(path)?;
    let bytes_read = content.len() as u64;
    let mut items = Vec::new();
    let mut raw_lines = Vec::new();
    let mut stats = RunStats::default();
    let mut has_result = false;
    let mut session_id = None;
    let mut started_at = None;

    for (i, line) in content.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        stats.total_lines += 1;
        raw_lines.push(line.to_string());

        match parser::parse_line(line) {
            parser::ParseResult::Parsed(new_items, meta) => {
                if session_id.is_none() {
                    session_id = meta.session_id.clone();
                }
                if started_at.is_none() {
                    started_at = meta.timestamp;
                }
                for item in &new_items {
                    match item {
                        TranscriptItem::ToolUse { .. } => stats.tool_calls += 1,
                        TranscriptItem::AssistantText { text, .. } => {
                            stats.assistant_chars += text.len()
                        }
                        TranscriptItem::SubagentStart { .. } => stats.subagent_spawns += 1,
                        _ => {}
                    }
                }
                if let Some(result) = meta.session_result {
                    has_result = true;
                    stats.cost_usd = Some(result.total_cost_usd);
                    let _ = tx.send(AppEvent::RunCompleted { run_id, result });
                }
                items.extend(new_items);
            }
            parser::ParseResult::Skipped => {}
            parser::ParseResult::Error(err) => {
                stats.parse_errors += 1;
                let _ = tx.send(AppEvent::ParseError {
                    run_id,
                    line_no: i,
                    error: err,
                });
            }
        }
    }

    if !items.is_empty() || !raw_lines.is_empty() {
        let _ = tx.send(AppEvent::RunUpdated {
            run_id,
            new_items: items,
            raw_lines,
            stats_delta: stats,
            session_id,
            started_at,
        });
    }

    Ok(WatchedFile {
        run_id,
        bytes_read,
        has_result,
    })
}

fn parse_file_incremental(
    wf: &mut WatchedFile,
    path: &PathBuf,
    tx: &UnboundedSender<AppEvent>,
    delta_acc: &mut DeltaAccumulator,
    is_active: bool,
) -> color_eyre::Result<()> {
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len();

    // Handle truncation — reset to re-parse from scratch
    if file_size < wf.bytes_read {
        wf.bytes_read = 0;
    }

    if file_size == wf.bytes_read {
        return Ok(());
    }

    let content = std::fs::read_to_string(path)?;
    // Guard: content may be shorter than bytes_read if file changed between stat and read
    let byte_offset = (wf.bytes_read as usize).min(content.len());
    let new_content = &content[byte_offset..];
    let mut items = Vec::new();
    let mut raw_lines = Vec::new();
    let mut stats = RunStats::default();
    let mut session_id = None;
    let mut started_at = None;
    let start_line = content[..byte_offset].lines().count();

    for (i, line) in new_content.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        stats.total_lines += 1;
        raw_lines.push(line.to_string());

        // For the active run, try Tier 2 streaming first
        if is_active {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if v.get("type").and_then(|t| t.as_str()) == Some("stream_event") {
                    if let Some(event) = streaming::process_stream_event(&v, wf.run_id, delta_acc) {
                        let _ = tx.send(event);
                    }
                    continue;
                }
            }
        }

        match parser::parse_line(line) {
            parser::ParseResult::Parsed(new_items, meta) => {
                if session_id.is_none() {
                    session_id = meta.session_id.clone();
                }
                if started_at.is_none() {
                    started_at = meta.timestamp;
                }
                for item in &new_items {
                    match item {
                        TranscriptItem::ToolUse { .. } => stats.tool_calls += 1,
                        TranscriptItem::AssistantText { text, .. } => {
                            stats.assistant_chars += text.len()
                        }
                        TranscriptItem::SubagentStart { .. } => stats.subagent_spawns += 1,
                        _ => {}
                    }
                }
                if let Some(result) = meta.session_result {
                    wf.has_result = true;
                    stats.cost_usd = Some(result.total_cost_usd);
                    let _ = tx.send(AppEvent::RunCompleted {
                        run_id: wf.run_id,
                        result,
                    });
                }
                items.extend(new_items);
            }
            parser::ParseResult::Skipped => {}
            parser::ParseResult::Error(err) => {
                stats.parse_errors += 1;
                let _ = tx.send(AppEvent::ParseError {
                    run_id: wf.run_id,
                    line_no: start_line + i,
                    error: err,
                });
            }
        }
    }

    wf.bytes_read = file_size;

    if !items.is_empty() || !raw_lines.is_empty() {
        let _ = tx.send(AppEvent::RunUpdated {
            run_id: wf.run_id,
            new_items: items,
            raw_lines,
            stats_delta: stats,
            session_id,
            started_at,
        });
    }

    Ok(())
}

pub async fn run_watcher(
    dir: PathBuf,
    tx: UnboundedSender<AppEvent>,
    cancel: CancellationToken,
) -> color_eyre::Result<()> {
    // 1. Discover existing files
    let files = discover_files(&dir)?;
    let mut tracked: HashMap<PathBuf, WatchedFile> = HashMap::new();
    let mut delta_acc = DeltaAccumulator::new();
    let mut next_run_id = 0usize;

    for path in &files {
        let run_id = next_run_id;
        next_run_id += 1;
        let _ = tx.send(AppEvent::RunDiscovered {
            run_id,
            path: path.clone(),
        });
        let wf = parse_file_initial(run_id, path, &tx)?;
        tracked.insert(path.clone(), wf);
    }

    // 2. Setup notify watcher — use std::sync::mpsc as bridge
    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = notify_tx.send(res);
        },
        notify::Config::default(),
    )?;
    watcher.watch(&dir, RecursiveMode::NonRecursive)?;

    // 3. Bridge notify's std channel to tokio async channel
    let (async_tx, mut async_rx) =
        tokio::sync::mpsc::unbounded_channel::<notify::Result<notify::Event>>();
    std::thread::spawn(move || {
        while let Ok(event) = notify_rx.recv() {
            if async_tx.send(event).is_err() {
                break;
            }
        }
    });

    // 4. Main event loop
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            Some(Ok(_event)) = async_rx.recv() => {
                // Coalesce: wait briefly and drain any backlog to avoid redundant re-scans
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                while async_rx.try_recv().is_ok() {}

                // Re-scan directory for new or modified files
                let current_files = discover_files(&dir).unwrap_or_default();

                for path in &current_files {
                    if let Some(wf) = tracked.get_mut(path) {
                        // Existing file — incremental parse
                        let is_active = !wf.has_result;
                        let _ = parse_file_incremental(wf, path, &tx, &mut delta_acc, is_active);
                    } else {
                        // New file discovered after startup
                        let run_id = next_run_id;
                        next_run_id += 1;
                        let _ = tx.send(AppEvent::RunDiscovered {
                            run_id,
                            path: path.clone(),
                        });
                        if let Ok(wf) = parse_file_initial(run_id, path, &tx) {
                            tracked.insert(path.clone(), wf);
                        }
                    }
                }

                // Prune tracked entries for deleted files
                tracked.retain(|path, _| current_files.contains(path));
            }
        }
    }

    Ok(())
}
