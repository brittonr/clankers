//! Prompt history search — Ctrl+R reverse incremental search
//!
//! SQLite-backed search across all sessions. Maintains an index of
//! user prompts for fast prefix/substring matching.

use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::Paragraph;

/// Maximum number of history entries to keep in memory
const MAX_MEMORY_ENTRIES: usize = 5000;
/// Maximum search results to show
const MAX_RESULTS: usize = 15;

/// A single history entry
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// The prompt text
    pub text: String,
    /// When this was submitted (unix timestamp)
    pub timestamp: i64,
    /// Session ID this came from
    pub session_id: String,
}

/// History storage backend.
/// Uses a simple JSONL file for persistence (SQLite could be added later).
pub struct HistoryStore {
    entries: VecDeque<HistoryEntry>,
    path: PathBuf,
    dirty: bool,
}

impl HistoryStore {
    /// Create or open a history store at the given path
    pub fn open(path: &Path) -> Self {
        let entries = Self::load_entries(path);
        Self {
            entries,
            path: path.to_path_buf(),
            dirty: false,
        }
    }

    /// Create an in-memory store (no persistence)
    pub fn in_memory() -> Self {
        Self {
            entries: VecDeque::new(),
            path: PathBuf::new(),
            dirty: false,
        }
    }

    fn load_entries(path: &Path) -> VecDeque<HistoryEntry> {
        let mut entries = VecDeque::new();
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                    entries.push_back(HistoryEntry {
                        text: entry["text"].as_str().unwrap_or("").to_string(),
                        timestamp: entry["ts"].as_i64().unwrap_or(0),
                        session_id: entry["sid"].as_str().unwrap_or("").to_string(),
                    });
                }
            }
        }
        // Keep only the most recent entries
        while entries.len() > MAX_MEMORY_ENTRIES {
            entries.pop_front();
        }
        entries
    }

    /// Add a new entry to the history
    pub fn add(&mut self, text: String, session_id: String) {
        // Deduplicate consecutive identical entries
        if let Some(last) = self.entries.back()
            && last.text == text
        {
            return;
        }

        let entry = HistoryEntry {
            text,
            timestamp: chrono::Utc::now().timestamp(),
            session_id,
        };

        self.entries.push_back(entry);
        self.dirty = true;

        // Trim if needed
        while self.entries.len() > MAX_MEMORY_ENTRIES {
            self.entries.pop_front();
        }
    }

    /// Search history for entries matching the query (substring match)
    /// Returns newest first.
    pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        if query.is_empty() {
            // Return most recent entries
            return self.entries.iter().rev().take(MAX_RESULTS).collect();
        }

        let lower_query = query.to_lowercase();
        self.entries
            .iter()
            .rev()
            .filter(|e| e.text.to_lowercase().contains(&lower_query))
            .take(MAX_RESULTS)
            .collect()
    }

    /// Flush dirty entries to disk
    pub fn flush(&mut self) {
        if !self.dirty || self.path.as_os_str().is_empty() {
            return;
        }

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Write all entries as JSONL
        let mut content = String::new();
        for entry in &self.entries {
            let json = serde_json::json!({
                "text": entry.text,
                "ts": entry.timestamp,
                "sid": entry.session_id,
            });
            content.push_str(&json.to_string());
            content.push('\n');
        }

        if let Err(e) = std::fs::write(&self.path, &content) {
            tracing::warn!("Failed to write history to {}: {}", self.path.display(), e);
        } else {
            self.dirty = false;
        }
    }

    /// Number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The interactive search widget state
pub struct HistorySearch {
    /// Whether the search is currently active
    pub active: bool,
    /// Current search query
    pub query: String,
    /// Currently selected result index (0 = newest match)
    pub selected: usize,
    /// Cached search results (indices into the store)
    results: Vec<String>,
}

impl HistorySearch {
    pub fn new() -> Self {
        Self {
            active: false,
            query: String::new(),
            selected: 0,
            results: Vec::new(),
        }
    }

    /// Activate the search overlay
    pub fn activate(&mut self) {
        self.active = true;
        self.query.clear();
        self.selected = 0;
        self.results.clear();
    }

    /// Deactivate and return the selected entry (if any)
    pub fn deactivate(&mut self) -> Option<String> {
        self.active = false;
        self.results.get(self.selected).cloned()
    }

    /// Cancel the search (no selection)
    pub fn cancel(&mut self) {
        self.active = false;
        self.query.clear();
        self.results.clear();
    }

    /// Type a character into the search query
    pub fn type_char(&mut self, c: char, store: &HistoryStore) {
        self.query.push(c);
        self.refresh_results(store);
    }

    /// Delete a character from the search query
    pub fn backspace(&mut self, store: &HistoryStore) {
        self.query.pop();
        self.refresh_results(store);
    }

    /// Move selection up (older results)
    pub fn select_prev(&mut self) {
        if self.selected + 1 < self.results.len() {
            self.selected += 1;
        }
    }

    /// Move selection down (newer results)
    pub fn select_next(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Refresh search results from the store
    fn refresh_results(&mut self, store: &HistoryStore) {
        self.results = store.search(&self.query).iter().map(|e| e.text.clone()).collect();
        self.selected = 0;
    }

    /// Initial population from store
    pub fn populate(&mut self, store: &HistoryStore) {
        self.refresh_results(store);
    }

    /// Render the search overlay
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.active {
            return;
        }

        // Position: bottom of screen, above the editor
        let width = (area.width - 4).min(80);
        let height = (MAX_RESULTS as u16 + 3).min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = area.height.saturating_sub(height + 2);
        let popup = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup);
        let block = Block::default()
            .title(" History Search (Ctrl+R) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        if inner.height < 2 {
            return;
        }

        // Search input line
        let search_line = Line::from(vec![
            Span::styled("search: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&self.query, Style::default().fg(Color::White)),
            Span::styled("_", Style::default().fg(Color::White).add_modifier(Modifier::SLOW_BLINK)),
        ]);
        let search_area = Rect::new(inner.x, inner.y, inner.width, 1);
        frame.render_widget(Paragraph::new(search_line), search_area);

        // Results list
        if inner.height > 1 {
            let results_area = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
            let items: Vec<ListItem> = self
                .results
                .iter()
                .enumerate()
                .take(results_area.height as usize)
                .map(|(i, text)| {
                    let style = if i == self.selected {
                        Style::default().fg(Color::Black).bg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    // Truncate long entries
                    let max_w = results_area.width.saturating_sub(2) as usize;
                    let display = if text.len() > max_w {
                        format!("{}…", &text[..max_w.saturating_sub(1)])
                    } else {
                        text.clone()
                    };
                    ListItem::new(Line::from(Span::styled(display, style)))
                })
                .collect();

            frame.render_widget(List::new(items), results_area);
        }
    }
}

impl Default for HistorySearch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_store_add_and_search() {
        let mut store = HistoryStore::in_memory();
        store.add("hello world".to_string(), "sess1".to_string());
        store.add("fix the bug".to_string(), "sess1".to_string());
        store.add("hello again".to_string(), "sess2".to_string());

        let results = store.search("hello");
        assert_eq!(results.len(), 2);
        // Newest first
        assert_eq!(results[0].text, "hello again");
        assert_eq!(results[1].text, "hello world");
    }

    #[test]
    fn test_dedup_consecutive() {
        let mut store = HistoryStore::in_memory();
        store.add("same".to_string(), "s1".to_string());
        store.add("same".to_string(), "s1".to_string());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_empty_search_returns_recent() {
        let mut store = HistoryStore::in_memory();
        for i in 0..20 {
            store.add(format!("entry {}", i), "s1".to_string());
        }
        let results = store.search("");
        assert_eq!(results.len(), MAX_RESULTS);
    }

    #[test]
    fn test_search_case_insensitive() {
        let mut store = HistoryStore::in_memory();
        store.add("Fix the BUG".to_string(), "s1".to_string());
        let results = store.search("bug");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_history_search_widget() {
        let mut search = HistorySearch::new();
        assert!(!search.active);
        search.activate();
        assert!(search.active);
        search.cancel();
        assert!(!search.active);
    }

    #[test]
    fn test_select_navigation() {
        let mut search = HistorySearch::new();
        search.results = vec!["a".into(), "b".into(), "c".into()];
        search.selected = 0;

        search.select_prev();
        assert_eq!(search.selected, 1);
        search.select_prev();
        assert_eq!(search.selected, 2);
        search.select_prev(); // Should clamp
        assert_eq!(search.selected, 2);

        search.select_next();
        assert_eq!(search.selected, 1);
        search.select_next();
        assert_eq!(search.selected, 0);
        search.select_next(); // Should clamp
        assert_eq!(search.selected, 0);
    }
}
