//! Multi-line input editor with history

use std::collections::VecDeque;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

const MAX_HISTORY: usize = 100;

/// Multi-line text editor with cursor and history
#[derive(Debug, Clone)]
pub struct Editor {
    lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
    history: VecDeque<String>,
    history_index: Option<usize>,
    saved_input: Option<Vec<String>>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            history: VecDeque::new(),
            history_index: None,
            saved_input: None,
        }
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn insert_char(&mut self, c: char) {
        if c == '\n' {
            let current = &self.lines[self.cursor_line];
            let remainder = current[self.cursor_col..].to_string();
            self.lines[self.cursor_line].truncate(self.cursor_col);
            self.lines.insert(self.cursor_line + 1, remainder);
            self.cursor_line += 1;
            self.cursor_col = 0;
        } else {
            self.lines[self.cursor_line].insert(self.cursor_col, c);
            self.cursor_col += c.len_utf8();
        }
        self.history_index = None;
    }

    /// Insert a string at the cursor position (used for paste operations)
    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            // Skip carriage returns – they are invisible control chars that
            // mess up rendering when pasted from Windows-style line endings.
            if c == '\r' {
                continue;
            }
            self.insert_char(c);
        }
    }

    pub fn delete_back(&mut self) {
        if self.cursor_col > 0 {
            // Find the previous char boundary
            let line = &self.lines[self.cursor_line];
            let prev = line[..self.cursor_col].char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            self.lines[self.cursor_line].remove(prev);
            self.cursor_col = prev;
        } else if self.cursor_line > 0 {
            let current = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
            self.lines[self.cursor_line].push_str(&current);
        }
        self.history_index = None;
    }

    pub fn delete_forward(&mut self) {
        if self.cursor_col < self.lines[self.cursor_line].len() {
            self.lines[self.cursor_line].remove(self.cursor_col);
        } else if self.cursor_line < self.lines.len() - 1 {
            let next = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next);
        }
        self.history_index = None;
    }

    pub fn delete_word_back(&mut self) {
        let line = &self.lines[self.cursor_line];
        let before_cursor = &line[..self.cursor_col];

        // Find start of word
        let mut new_pos = self.cursor_col;
        let mut found_word = false;
        for (i, c) in before_cursor.char_indices().rev() {
            if c.is_whitespace() {
                if found_word {
                    new_pos = i + 1;
                    break;
                }
            } else {
                found_word = true;
            }
            new_pos = i;
        }

        self.lines[self.cursor_line].replace_range(new_pos..self.cursor_col, "");
        self.cursor_col = new_pos;
        self.history_index = None;
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            let line = &self.lines[self.cursor_line];
            self.cursor_col = line[..self.cursor_col].char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor_col < self.lines[self.cursor_line].len() {
            let line = &self.lines[self.cursor_line];
            self.cursor_col = line[self.cursor_col..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_col + i)
                .unwrap_or(line.len());
        } else if self.cursor_line < self.lines.len() - 1 {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_line].len();
    }

    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index.is_none() {
            self.saved_input = Some(self.lines.clone());
            self.history_index = Some(self.history.len() - 1);
        } else if let Some(idx) = self.history_index
            && idx > 0
        {
            self.history_index = Some(idx - 1);
        }

        if let Some(idx) = self.history_index {
            let text = &self.history[idx];
            self.lines = text.lines().map(|s| s.to_string()).collect();
            if self.lines.is_empty() {
                self.lines.push(String::new());
            }
            self.cursor_line = self.lines.len() - 1;
            self.cursor_col = self.lines[self.cursor_line].len();
        }
    }

    pub fn history_down(&mut self) {
        if let Some(idx) = self.history_index {
            if idx < self.history.len() - 1 {
                self.history_index = Some(idx + 1);
                let text = &self.history[idx + 1];
                self.lines = text.lines().map(|s| s.to_string()).collect();
                if self.lines.is_empty() {
                    self.lines.push(String::new());
                }
            } else {
                self.history_index = None;
                if let Some(saved) = self.saved_input.take() {
                    self.lines = saved;
                }
            }
            self.cursor_line = self.lines.len() - 1;
            self.cursor_col = self.lines[self.cursor_line].len();
        }
    }

    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.history_index = None;
    }

    pub fn submit(&mut self) -> Option<String> {
        let text = self.lines.join("\n");
        if text.trim().is_empty() {
            return None;
        }
        self.history.push_back(text.clone());
        if self.history.len() > MAX_HISTORY {
            self.history.pop_front();
        }
        self.clear();
        Some(text)
    }

    pub fn content(&self) -> &[String] {
        &self.lines
    }

    /// Move the cursor to the position corresponding to a mouse click
    /// inside the editor's inner area. `rel_col` and `rel_row` are relative
    /// to the inner area (after removing borders). `inner_width` is the
    /// available character width and `indicator_len` is the prompt prefix
    /// length on the first line.
    pub fn click_to_cursor(&mut self, rel_col: u16, rel_row: u16, inner_width: usize, indicator_len: usize) {
        if inner_width == 0 {
            return;
        }

        let target_visual_row = rel_row as usize;
        let target_col = rel_col as usize;
        let mut visual_row: usize = 0;

        for (line_idx, line) in self.lines.iter().enumerate() {
            let prefix_len = if line_idx == 0 { indicator_len } else { 2 };
            let content_len = prefix_len + line.len();
            let wrapped_rows = if content_len == 0 {
                1
            } else {
                content_len.div_ceil(inner_width)
            };

            if target_visual_row < visual_row + wrapped_rows {
                // The click is in this logical line
                let row_within_line = target_visual_row - visual_row;
                let abs_offset = row_within_line * inner_width + target_col;
                let char_offset = abs_offset.saturating_sub(prefix_len);
                self.cursor_line = line_idx;
                self.cursor_col = char_offset.min(line.len());
                // Snap to a char boundary
                while self.cursor_col > 0 && !line.is_char_boundary(self.cursor_col) {
                    self.cursor_col -= 1;
                }
                return;
            }
            visual_row += wrapped_rows;
        }

        // Click is past the last line — put cursor at the end
        self.cursor_line = self.lines.len() - 1;
        self.cursor_col = self.lines[self.cursor_line].len();
    }

    /// Check if the editor is empty (no meaningful content)
    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.is_empty())
    }

    /// Compute the number of visual (wrapped) lines given an available width.
    /// `indicator_len` is the length of the prompt indicator on the first line.
    pub fn visual_line_count(&self, width: usize, indicator_len: usize) -> usize {
        if width == 0 {
            return self.lines.len();
        }
        self.lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let prefix_len = if i == 0 { indicator_len } else { 2 }; // "  " indent
                let content_len = prefix_len + line.len();
                if content_len == 0 {
                    1
                } else {
                    content_len.div_ceil(width) // ceil division
                }
            })
            .sum()
    }

    /// Compute the visual (x, y) cursor position accounting for wrapping.
    /// Returns (col, row) relative to the inner area of the editor widget.
    pub fn visual_cursor_position(&self, width: usize, indicator_len: usize) -> (u16, u16) {
        if width == 0 {
            return (0, 0);
        }

        let mut visual_row: usize = 0;

        // Count visual rows from lines before the cursor line
        for (i, line) in self.lines.iter().enumerate() {
            if i == self.cursor_line {
                break;
            }
            let prefix_len = if i == 0 { indicator_len } else { 2 };
            let content_len = prefix_len + line.len();
            visual_row += if content_len == 0 {
                1
            } else {
                content_len.div_ceil(width)
            };
        }

        // Now compute position within the cursor line
        let prefix_len = if self.cursor_line == 0 { indicator_len } else { 2 };
        let offset_in_line = prefix_len + self.cursor_col;
        visual_row += offset_in_line / width;
        let visual_col = offset_in_line % width;

        (visual_col as u16, visual_row as u16)
    }
}

/// Render the editor widget
pub fn render_editor(
    frame: &mut Frame,
    editor: &Editor,
    area: Rect,
    indicator: &str,
    border_color: Color,
    title: &str,
) {
    let lines: Vec<Line> = editor
        .content()
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                Line::from(vec![Span::raw(indicator), Span::raw(line.as_str())])
            } else {
                Line::from(Span::raw(format!("  {}", line)))
            }
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title.to_string());

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);

    // Place the cursor accounting for wrapping and borders
    let inner_width = area.width.saturating_sub(2) as usize; // subtract left+right border
    let (cx, cy) = editor.visual_cursor_position(inner_width, indicator.len());
    frame.set_cursor_position((
        area.x + 1 + cx, // +1 for left border
        area.y + 1 + cy, // +1 for top border
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_editor_empty() {
        let editor = Editor::new();
        assert_eq!(editor.line_count(), 1);
        assert_eq!(editor.cursor_line, 0);
        assert_eq!(editor.cursor_col, 0);
        assert_eq!(editor.content()[0], "");
    }

    #[test]
    fn test_insert_char_single_line() {
        let mut editor = Editor::new();
        editor.insert_char('h');
        editor.insert_char('i');
        assert_eq!(editor.content()[0], "hi");
        assert_eq!(editor.cursor_col, 2);
    }

    #[test]
    fn test_insert_newline() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('\n');
        editor.insert_char('c');

        assert_eq!(editor.line_count(), 2);
        assert_eq!(editor.content()[0], "ab");
        assert_eq!(editor.content()[1], "c");
        assert_eq!(editor.cursor_line, 1);
        assert_eq!(editor.cursor_col, 1);
    }

    #[test]
    fn test_delete_back_single_line() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.delete_back();

        assert_eq!(editor.content()[0], "a");
        assert_eq!(editor.cursor_col, 1);
    }

    #[test]
    fn test_delete_back_empty() {
        let mut editor = Editor::new();
        editor.delete_back(); // Shouldn't crash
        assert_eq!(editor.cursor_col, 0);
    }

    #[test]
    fn test_delete_back_join_lines() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('\n');
        editor.insert_char('b');

        // Cursor at start of line 1
        editor.cursor_line = 1;
        editor.cursor_col = 0;
        editor.delete_back();

        assert_eq!(editor.line_count(), 1);
        assert_eq!(editor.content()[0], "ab");
        assert_eq!(editor.cursor_line, 0);
    }

    #[test]
    fn test_delete_forward() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('c');
        editor.cursor_col = 1; // Between 'a' and 'b'
        editor.delete_forward();

        assert_eq!(editor.content()[0], "ac");
        assert_eq!(editor.cursor_col, 1);
    }

    #[test]
    fn test_delete_forward_join_lines() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('\n');
        editor.insert_char('b');

        // Move cursor to end of first line
        editor.cursor_line = 0;
        editor.cursor_col = 1;
        editor.delete_forward();

        assert_eq!(editor.line_count(), 1);
        assert_eq!(editor.content()[0], "ab");
    }

    #[test]
    fn test_move_left_right() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('c');

        assert_eq!(editor.cursor_col, 3);
        editor.move_left();
        assert_eq!(editor.cursor_col, 2);
        editor.move_left();
        assert_eq!(editor.cursor_col, 1);
        editor.move_right();
        assert_eq!(editor.cursor_col, 2);
    }

    #[test]
    fn test_move_left_across_lines() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('\n');
        editor.insert_char('b');

        // At position 1 of line 1
        assert_eq!(editor.cursor_line, 1);
        assert_eq!(editor.cursor_col, 1);

        editor.move_left();
        editor.move_left(); // Should go to end of previous line

        assert_eq!(editor.cursor_line, 0);
        assert_eq!(editor.cursor_col, 1);
    }

    #[test]
    fn test_move_right_across_lines() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('\n');
        editor.insert_char('b');

        editor.cursor_line = 0;
        editor.cursor_col = 1; // End of first line
        editor.move_right();

        assert_eq!(editor.cursor_line, 1);
        assert_eq!(editor.cursor_col, 0);
    }

    #[test]
    fn test_move_home_end() {
        let mut editor = Editor::new();
        editor.insert_char('h');
        editor.insert_char('e');
        editor.insert_char('l');
        editor.insert_char('l');
        editor.insert_char('o');

        assert_eq!(editor.cursor_col, 5);

        editor.move_home();
        assert_eq!(editor.cursor_col, 0);

        editor.move_end();
        assert_eq!(editor.cursor_col, 5);
    }

    #[test]
    fn test_clear() {
        let mut editor = Editor::new();
        editor.insert_char('t');
        editor.insert_char('e');
        editor.insert_char('s');
        editor.insert_char('t');
        editor.clear();

        assert_eq!(editor.line_count(), 1);
        assert_eq!(editor.content()[0], "");
        assert_eq!(editor.cursor_line, 0);
        assert_eq!(editor.cursor_col, 0);
    }

    #[test]
    fn test_submit_returns_content() {
        let mut editor = Editor::new();
        editor.insert_char('h');
        editor.insert_char('i');
        editor.insert_char('\n');
        editor.insert_char('t');
        editor.insert_char('h');
        editor.insert_char('e');
        editor.insert_char('r');
        editor.insert_char('e');

        let result = editor.submit();
        assert!(result.is_some());

        // Should be cleared after submit
        assert_eq!(editor.line_count(), 1);
        assert_eq!(editor.content()[0], "");
    }

    #[test]
    fn test_submit_empty_returns_none() {
        let mut editor = Editor::new();
        let result = editor.submit();
        assert!(result.is_none());
    }

    #[test]
    fn test_submit_whitespace_only_returns_none() {
        let mut editor = Editor::new();
        editor.insert_char(' ');
        editor.insert_char('\n');
        editor.insert_char(' ');

        let result = editor.submit();
        assert!(result.is_none());
    }

    #[test]
    fn test_history_up_down() {
        let mut editor = Editor::new();

        // Submit first entry
        editor.insert_char('f');
        editor.insert_char('i');
        editor.insert_char('r');
        editor.insert_char('s');
        editor.insert_char('t');
        editor.submit();

        // Submit second entry
        editor.insert_char('s');
        editor.insert_char('e');
        editor.insert_char('c');
        editor.insert_char('o');
        editor.insert_char('n');
        editor.insert_char('d');
        editor.submit();

        // Navigate history
        editor.history_up();
        assert_eq!(editor.content()[0], "second");

        editor.history_up();
        assert_eq!(editor.content()[0], "first");

        editor.history_down();
        assert_eq!(editor.content()[0], "second");

        editor.history_down();
        assert_eq!(editor.content()[0], ""); // Back to empty
    }

    #[test]
    fn test_history_saves_current_input() {
        let mut editor = Editor::new();
        editor.insert_char('s');
        editor.insert_char('a');
        editor.insert_char('v');
        editor.insert_char('e');
        editor.insert_char('d');
        editor.submit();

        editor.insert_char('c');
        editor.insert_char('u');
        editor.insert_char('r');
        editor.insert_char('r');
        editor.insert_char('e');
        editor.insert_char('n');
        editor.insert_char('t');

        // Go up then down should restore current input
        editor.history_up();
        editor.history_down();

        assert_eq!(editor.content()[0], "current");
    }

    #[test]
    fn test_delete_word_back() {
        let mut editor = Editor::new();
        editor.insert_char('h');
        editor.insert_char('e');
        editor.insert_char('l');
        editor.insert_char('l');
        editor.insert_char('o');
        editor.insert_char(' ');
        editor.insert_char('w');
        editor.insert_char('o');
        editor.insert_char('r');
        editor.insert_char('l');
        editor.insert_char('d');

        editor.delete_word_back();
        assert_eq!(editor.content()[0], "hello ");

        editor.delete_word_back();
        assert_eq!(editor.content()[0], "");
    }

    #[test]
    fn test_history_max_size() {
        let mut editor = Editor::new();

        // Add more than MAX_HISTORY entries
        for _ in 0..150 {
            editor.insert_char('a');
            editor.submit();
        }

        assert!(editor.history.len() <= MAX_HISTORY);
    }

    #[test]
    fn test_visual_line_count_no_wrap() {
        let mut editor = Editor::new();
        editor.insert_char('h');
        editor.insert_char('i');
        // "  hi" with indicator "> " = "> hi" = 4 chars, width 80 = 1 visual line
        assert_eq!(editor.visual_line_count(80, 2), 1);
    }

    #[test]
    fn test_visual_line_count_wraps() {
        let mut editor = Editor::new();
        // Type 20 chars on first line: "> " + 20 chars = 22 chars total
        for _ in 0..20 {
            editor.insert_char('a');
        }
        // Width 10: ceil(22/10) = 3 visual lines
        assert_eq!(editor.visual_line_count(10, 2), 3);
    }

    #[test]
    fn test_visual_line_count_multiline_wraps() {
        let mut editor = Editor::new();
        // Line 0: "> " + 8 chars = 10 chars -> 1 visual line at width 10
        for _ in 0..8 {
            editor.insert_char('a');
        }
        editor.insert_char('\n');
        // Line 1: "  " + 15 chars = 17 chars -> 2 visual lines at width 10
        for _ in 0..15 {
            editor.insert_char('b');
        }
        assert_eq!(editor.visual_line_count(10, 2), 3);
    }

    #[test]
    fn test_visual_cursor_position_no_wrap() {
        let mut editor = Editor::new();
        editor.insert_char('h');
        editor.insert_char('i');
        // Cursor at col 2, with indicator "> " = offset 4 in visual line
        let (cx, cy) = editor.visual_cursor_position(80, 2);
        assert_eq!(cx, 4);
        assert_eq!(cy, 0);
    }

    #[test]
    fn test_visual_cursor_position_wrapped() {
        let mut editor = Editor::new();
        // Type 12 chars: "> " + 12 = 14 chars. At width 10: row 0 has 10, row 1 has 4
        for _ in 0..12 {
            editor.insert_char('x');
        }
        let (cx, cy) = editor.visual_cursor_position(10, 2);
        assert_eq!(cy, 1);
        assert_eq!(cx, 4);
    }

    #[test]
    fn test_visual_cursor_position_second_line() {
        let mut editor = Editor::new();
        editor.insert_char('a');
        editor.insert_char('\n');
        editor.insert_char('b');
        editor.insert_char('c');
        // Line 0: "> a" = 3 chars -> 1 visual line at width 80
        // Line 1: "  bc" cursor at col 2 -> offset = 2 + 2 = 4
        let (cx, cy) = editor.visual_cursor_position(80, 2);
        assert_eq!(cy, 1);
        assert_eq!(cx, 4);
    }

    #[test]
    fn test_click_to_cursor_first_line() {
        let mut editor = Editor::new();
        // "hello" with indicator "> " → "> hello"
        for c in "hello".chars() {
            editor.insert_char(c);
        }
        // Click at column 4 of visual row 0 → past the ">" prefix (len 2), col 2 in text
        editor.click_to_cursor(4, 0, 80, 2);
        assert_eq!(editor.cursor_line, 0);
        assert_eq!(editor.cursor_col, 2); // "he|llo"
    }

    #[test]
    fn test_click_to_cursor_second_line() {
        let mut editor = Editor::new();
        for c in "abc".chars() {
            editor.insert_char(c);
        }
        editor.insert_char('\n');
        for c in "defgh".chars() {
            editor.insert_char(c);
        }
        // Line 0: "> abc" (1 visual line at width 80)
        // Line 1: "  defgh" (prefix "  " = 2)
        // Click visual row 1, col 5 → offset 5 - 2 = 3 → "def|gh"
        editor.click_to_cursor(5, 1, 80, 2);
        assert_eq!(editor.cursor_line, 1);
        assert_eq!(editor.cursor_col, 3);
    }

    #[test]
    fn test_click_to_cursor_past_end() {
        let mut editor = Editor::new();
        for c in "hi".chars() {
            editor.insert_char(c);
        }
        // Click way past the content → cursor should be at the end
        editor.click_to_cursor(50, 5, 80, 2);
        assert_eq!(editor.cursor_line, 0);
        assert_eq!(editor.cursor_col, 2);
    }

    #[test]
    fn test_click_to_cursor_wrapped_line() {
        let mut editor = Editor::new();
        // Type 15 chars: "> " + 15 = 17 chars. Width 10: row 0 = 10 chars, row 1 = 7 chars
        for _ in 0..15 {
            editor.insert_char('a');
        }
        // Click at visual row 1, col 3 → abs offset = 1*10 + 3 = 13, minus prefix 2 = 11
        editor.click_to_cursor(3, 1, 10, 2);
        assert_eq!(editor.cursor_line, 0);
        assert_eq!(editor.cursor_col, 11);
    }
}
