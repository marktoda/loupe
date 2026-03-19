use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::events::{AppEvent, ViewMode, FocusPane};
use crate::run::{Run, RunStatus, TranscriptItem};
use crate::ui::search::SearchState;
use std::collections::HashSet;

pub struct App {
    pub runs: Vec<Run>,
    pub selected_run: Option<usize>,
    pub view_mode: ViewMode,
    pub focus: FocusPane,
    pub scroll_offset: usize,
    pub auto_follow: bool,
    pub search: SearchState,
    pub show_help: bool,
    pub should_quit: bool,
    pub dirty: bool,
    pub expanded_tools: HashSet<usize>,
}

impl App {
    pub fn new(view_mode: ViewMode) -> Self {
        Self {
            runs: Vec::new(),
            selected_run: None,
            view_mode,
            focus: FocusPane::MainViewer,
            scroll_offset: 0,
            auto_follow: true,
            search: SearchState::default(),
            show_help: false,
            should_quit: false,
            dirty: true,
            expanded_tools: HashSet::new(),
        }
    }

    pub fn update_state(&mut self, event: AppEvent) {
        match event {
            AppEvent::RunDiscovered { run_id, path } => {
                self.runs.push(Run::new(run_id, path));
                if self.auto_follow {
                    self.selected_run = Some(run_id);
                }
            }
            AppEvent::RunUpdated { run_id, new_items, raw_lines, stats_delta, session_id, started_at } => {
                if let Some(run) = self.runs.get_mut(run_id) {
                    run.items.extend(new_items);
                    run.raw_lines.extend(raw_lines);
                    run.stats.merge(&stats_delta);
                    run.status = RunStatus::Running;
                    run.last_modified = Some(std::time::SystemTime::now());
                    if session_id.is_some() && run.session_id.is_none() {
                        run.session_id = session_id;
                    }
                    if started_at.is_some() && run.started_at.is_none() {
                        run.started_at = started_at;
                    }
                }
            }
            AppEvent::RunCompleted { run_id, result } => {
                if let Some(run) = self.runs.get_mut(run_id) {
                    run.status = if result.is_error { RunStatus::Failed } else { RunStatus::Completed };
                    run.ended_at = Some(Utc::now());
                    run.stats.cost_usd = Some(result.total_cost_usd);
                    run.result = Some(result);
                }
            }
            AppEvent::StreamDelta { run_id, text } => {
                if let Some(run) = self.runs.get_mut(run_id) {
                    if let Some(last) = run.items.last_mut()
                        && matches!(last, TranscriptItem::AssistantText { is_partial: true, .. })
                    {
                        *last = TranscriptItem::AssistantText { text, is_partial: true };
                        return;
                    }
                    run.items.push(TranscriptItem::AssistantText { text, is_partial: true });
                }
            }
            AppEvent::StreamBlockDone { run_id, item } => {
                if let Some(run) = self.runs.get_mut(run_id) {
                    if let Some(last) = run.items.last_mut()
                        && matches!(last, TranscriptItem::AssistantText { is_partial: true, .. })
                    {
                        *last = item;
                        return;
                    }
                    run.items.push(item);
                }
            }
            AppEvent::ParseError { run_id, line_no, error } => {
                if let Some(run) = self.runs.get_mut(run_id) {
                    run.stats.parse_errors += 1;
                    run.items.push(TranscriptItem::Error { message: format!("Line {line_no}: {error}") });
                }
            }
            AppEvent::Key(_) | AppEvent::Resize(_, _) | AppEvent::Tick => {}
        }
        self.dirty = true;
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // Help overlay dismisses on any key
        if self.show_help {
            self.show_help = false;
            return;
        }

        // Search mode captures all input
        if self.search.is_active {
            match key.code {
                KeyCode::Esc => {
                    self.search.is_active = false;
                    self.search.highlights_visible = false;
                    self.search.query.clear();
                    self.search.matches.clear();
                }
                KeyCode::Enter => {
                    self.search.is_active = false;
                    self.search.highlights_visible = true;
                }
                KeyCode::Backspace => { self.search.query.pop(); self.recompute_search(); }
                KeyCode::Char(c) => { self.search.query.push(c); self.recompute_search(); }
                _ => {}
            }
            return;
        }

        // Global keys
        match key.code {
            KeyCode::Char('q') => { self.should_quit = true; return; }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true; return;
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    FocusPane::RunList => FocusPane::MainViewer,
                    FocusPane::MainViewer => FocusPane::RunList,
                };
                return;
            }
            KeyCode::Char('1') => { self.view_mode = ViewMode::Transcript; return; }
            KeyCode::Char('2') => { self.view_mode = ViewMode::Tools; return; }
            KeyCode::Char('3') => { self.view_mode = ViewMode::Raw; return; }
            KeyCode::Char('/') => { self.search.is_active = true; self.search.query.clear(); return; }
            KeyCode::Char('?') => { self.show_help = true; return; }
            _ => {}
        }

        // Focus-specific keys
        match self.focus {
            FocusPane::RunList => match key.code {
                KeyCode::Char('j') | KeyCode::Down => self.select_next_run(),
                KeyCode::Char('k') | KeyCode::Up => self.select_prev_run(),
                KeyCode::Char('g') => { if !self.runs.is_empty() { self.selected_run = Some(0); } }
                KeyCode::Char('G') => self.selected_run = self.runs.len().checked_sub(1),
                KeyCode::Char('f') => { self.jump_to_active_run(); self.auto_follow = true; }
                _ => {}
            },
            FocusPane::MainViewer => match key.code {
                KeyCode::Char('j') | KeyCode::Down => { self.scroll_offset = self.scroll_offset.saturating_add(1); self.auto_follow = false; }
                KeyCode::Char('k') | KeyCode::Up => { self.scroll_offset = self.scroll_offset.saturating_sub(1); self.auto_follow = false; }
                KeyCode::PageDown => { self.scroll_offset = self.scroll_offset.saturating_add(20); self.auto_follow = false; }
                KeyCode::PageUp => { self.scroll_offset = self.scroll_offset.saturating_sub(20); self.auto_follow = false; }
                KeyCode::Char('g') | KeyCode::Home => { self.scroll_offset = 0; self.auto_follow = false; }
                KeyCode::Char('G') | KeyCode::End => { self.scroll_to_bottom(); self.auto_follow = true; }
                KeyCode::Enter => self.toggle_tool_expansion(),
                KeyCode::Char('f') => { self.scroll_to_bottom(); self.auto_follow = true; }
                KeyCode::Char('n') if self.search.highlights_visible => self.search_next(),
                KeyCode::Char('N') if self.search.highlights_visible => self.search_prev(),
                _ => {}
            },
        }
    }

    pub fn check_active_run_timeout(&mut self) {
        let now = std::time::SystemTime::now();
        for run in &mut self.runs {
            if run.status == RunStatus::Running
                && let Some(last_mod) = run.last_modified
                && now.duration_since(last_mod).unwrap_or_default().as_secs() > 60
            {
                run.status = RunStatus::Unknown;
                self.dirty = true;
            }
        }
    }

    pub fn selected_run(&self) -> Option<&Run> {
        self.selected_run.and_then(|i| self.runs.get(i))
    }

    fn select_next_run(&mut self) {
        if self.runs.is_empty() { return; }
        let current = self.selected_run.unwrap_or(0);
        self.selected_run = Some((current + 1).min(self.runs.len() - 1));
        self.scroll_offset = 0;
        self.expanded_tools.clear();
    }

    fn select_prev_run(&mut self) {
        if self.runs.is_empty() { return; }
        let current = self.selected_run.unwrap_or(0);
        self.selected_run = Some(current.saturating_sub(1));
        self.scroll_offset = 0;
        self.expanded_tools.clear();
    }

    fn jump_to_active_run(&mut self) {
        // Find the last run with Running status
        if let Some(idx) = self.runs.iter().rposition(|r| r.status == RunStatus::Running) {
            self.selected_run = Some(idx);
            self.scroll_offset = 0;
        } else if let Some(last) = self.runs.len().checked_sub(1) {
            self.selected_run = Some(last);
        }
    }

    fn scroll_to_bottom(&mut self) {
        if let Some(run) = self.selected_run() {
            self.scroll_offset = run.items.len().saturating_sub(1);
        }
    }

    fn toggle_tool_expansion(&mut self) {
        // Toggle expansion of the item at current scroll position
        let is_tool = self.selected_run().and_then(|run| {
            run.items.get(self.scroll_offset)
                .map(|item| matches!(item, TranscriptItem::ToolUse { .. }))
        }).unwrap_or(false);
        if is_tool {
            if self.expanded_tools.contains(&self.scroll_offset) {
                self.expanded_tools.remove(&self.scroll_offset);
            } else {
                self.expanded_tools.insert(self.scroll_offset);
            }
        }
    }

    fn recompute_search(&mut self) {
        self.search.matches.clear();
        if self.search.query.is_empty() { return; }
        let query_lower = self.search.query.to_lowercase();
        // Collect matching indices without holding a borrow on self during mutation
        let matches: Vec<usize> = if let Some(run) = self.selected_run() {
            run.items.iter().enumerate().filter_map(|(i, item)| {
                let hit = match item {
                    TranscriptItem::AssistantText { text, .. } => text.to_lowercase().contains(&query_lower),
                    TranscriptItem::ToolUse { name, summary, .. } => {
                        name.to_lowercase().contains(&query_lower)
                            || summary.to_lowercase().contains(&query_lower)
                    }
                    TranscriptItem::ToolResult { summary, .. } => summary.to_lowercase().contains(&query_lower),
                    TranscriptItem::Error { message } => message.to_lowercase().contains(&query_lower),
                    TranscriptItem::SystemEvent { label, detail, .. } => {
                        label.to_lowercase().contains(&query_lower)
                            || detail.to_lowercase().contains(&query_lower)
                    }
                    _ => false,
                };
                if hit { Some(i) } else { None }
            }).collect()
        } else {
            Vec::new()
        };
        self.search.matches = matches;
        self.search.current_match = 0;
    }

    fn search_next(&mut self) {
        if self.search.matches.is_empty() { return; }
        self.search.current_match = (self.search.current_match + 1) % self.search.matches.len();
        self.scroll_offset = self.search.matches[self.search.current_match];
        self.auto_follow = false;
    }

    fn search_prev(&mut self) {
        if self.search.matches.is_empty() { return; }
        if self.search.current_match == 0 {
            self.search.current_match = self.search.matches.len() - 1;
        } else {
            self.search.current_match -= 1;
        }
        self.scroll_offset = self.search.matches[self.search.current_match];
        self.auto_follow = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::AppEvent;
    use crate::run::{TranscriptItem, RunStats};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn run_discovered_adds_to_list() {
        let mut app = App::new(ViewMode::Transcript);
        app.update_state(AppEvent::RunDiscovered { run_id: 0, path: "a.jsonl".into() });
        assert_eq!(app.runs.len(), 1);
        assert_eq!(app.selected_run, Some(0));
    }

    #[test]
    fn run_updated_appends_items() {
        let mut app = App::new(ViewMode::Transcript);
        app.update_state(AppEvent::RunDiscovered { run_id: 0, path: "a.jsonl".into() });
        app.update_state(AppEvent::RunUpdated {
            run_id: 0,
            new_items: vec![TranscriptItem::AssistantText { text: "hi".into(), is_partial: false }],
            raw_lines: vec![],
            stats_delta: RunStats { assistant_chars: 2, ..Default::default() },
            session_id: None,
            started_at: None,
        });
        assert_eq!(app.runs[0].items.len(), 1);
        assert_eq!(app.runs[0].stats.assistant_chars, 2);
    }

    #[test]
    fn stream_delta_creates_partial() {
        let mut app = App::new(ViewMode::Transcript);
        app.update_state(AppEvent::RunDiscovered { run_id: 0, path: "a.jsonl".into() });
        app.update_state(AppEvent::StreamDelta { run_id: 0, text: "Hello".into() });
        assert_eq!(app.runs[0].items.len(), 1);
        assert!(matches!(&app.runs[0].items[0], TranscriptItem::AssistantText { is_partial: true, .. }));
    }

    #[test]
    fn stream_block_done_replaces_partial() {
        let mut app = App::new(ViewMode::Transcript);
        app.update_state(AppEvent::RunDiscovered { run_id: 0, path: "a.jsonl".into() });
        app.update_state(AppEvent::StreamDelta { run_id: 0, text: "Hello".into() });
        app.update_state(AppEvent::StreamBlockDone {
            run_id: 0,
            item: TranscriptItem::AssistantText { text: "Hello world".into(), is_partial: false },
        });
        assert_eq!(app.runs[0].items.len(), 1);
        assert!(matches!(&app.runs[0].items[0], TranscriptItem::AssistantText { text, is_partial: false } if text == "Hello world"));
    }

    #[test]
    fn quit_on_q() {
        let mut app = App::new(ViewMode::Transcript);
        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(app.should_quit);
    }

    #[test]
    fn tab_toggles_focus() {
        let mut app = App::new(ViewMode::Transcript);
        assert_eq!(app.focus, FocusPane::MainViewer);
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusPane::RunList);
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusPane::MainViewer);
    }

    #[test]
    fn view_mode_switching() {
        let mut app = App::new(ViewMode::Transcript);
        app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        assert_eq!(app.view_mode, ViewMode::Tools);
        app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        assert_eq!(app.view_mode, ViewMode::Raw);
    }

    #[test]
    fn scroll_disables_auto_follow() {
        let mut app = App::new(ViewMode::Transcript);
        assert!(app.auto_follow);
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert!(!app.auto_follow);
    }

    #[test]
    fn g_uppercase_re_enables_auto_follow() {
        let mut app = App::new(ViewMode::Transcript);
        app.auto_follow = false;
        app.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
        assert!(app.auto_follow);
    }
}
