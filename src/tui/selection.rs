//! Mouse text selection and clipboard support

use std::io::Write;

use ratatui::layout::Rect;

/// A position in the rendered text (visual coordinates)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPos {
    /// Visual row (0-indexed, relative to the message area content, not screen)
    pub row: usize,
    /// Column (0-indexed)
    pub col: usize,
}

impl TextPos {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

impl PartialOrd for TextPos {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TextPos {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.row.cmp(&other.row).then(self.col.cmp(&other.col))
    }
}

/// Tracks an active or completed text selection
#[derive(Debug, Clone)]
pub struct TextSelection {
    /// Where the mouse was pressed down
    pub anchor: TextPos,
    /// Current drag position (moves as mouse drags)
    pub cursor: TextPos,
    /// Whether a drag is in progress
    pub active: bool,
}

impl TextSelection {
    /// Start a new selection at the given position
    pub fn start(pos: TextPos) -> Self {
        Self {
            anchor: pos,
            cursor: pos,
            active: true,
        }
    }

    /// Update the drag endpoint
    pub fn update(&mut self, pos: TextPos) {
        self.cursor = pos;
    }

    /// Finalize the selection (mouse released)
    pub fn finish(&mut self) {
        self.active = false;
    }

    /// Get the selection range as (start, end) with start <= end
    pub fn ordered(&self) -> (TextPos, TextPos) {
        if self.anchor <= self.cursor {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }

    /// Check if the selection is empty (zero-length)
    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }

    /// Check if a given row is within the selection range
    pub fn contains_row(&self, row: usize) -> bool {
        let (start, end) = self.ordered();
        row >= start.row && row <= end.row
    }

    /// For a given row, return the selected column range (start_col, end_col).
    /// Returns None if the row is not in the selection.
    pub fn col_range_for_row(&self, row: usize, line_len: usize) -> Option<(usize, usize)> {
        let (start, end) = self.ordered();
        if row < start.row || row > end.row {
            return None;
        }
        let col_start = if row == start.row { start.col } else { 0 };
        let col_end = if row == end.row {
            end.col.min(line_len)
        } else {
            line_len
        };
        if col_start >= col_end && row == start.row && row == end.row {
            return None;
        }
        Some((col_start, col_end))
    }

    /// Extract the selected text from rendered lines
    pub fn extract_text(&self, rendered_lines: &[String]) -> String {
        if self.is_empty() {
            return String::new();
        }
        let (start, end) = self.ordered();
        let mut result = String::new();

        for row in start.row..=end.row {
            if row >= rendered_lines.len() {
                break;
            }
            let line = &rendered_lines[row];
            let col_start = if row == start.row { start.col.min(line.len()) } else { 0 };
            let col_end = if row == end.row {
                end.col.min(line.len())
            } else {
                line.len()
            };

            if col_start <= col_end && col_start <= line.len() {
                // Handle char boundaries properly
                let safe_start = line
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain(std::iter::once(line.len()))
                    .find(|&i| i >= col_start)
                    .unwrap_or(line.len());
                let safe_end = line
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain(std::iter::once(line.len()))
                    .find(|&i| i >= col_end)
                    .unwrap_or(line.len());

                result.push_str(&line[safe_start..safe_end]);
            }

            if row < end.row {
                result.push('\n');
            }
        }

        result
    }
}

/// Convert a visual line position to a logical line index and column offset.
///
/// `visual_pos` is the absolute visual line (counting wrapped lines from the start).
/// `rendered_lines` are the logical (unwrapped) plain-text lines.
/// `inner_width` is the content width (inside borders) used for wrapping.
///
/// Returns `(logical_line, col_offset)` where `col_offset` accounts for
/// wrapped continuation lines within a single logical line.
pub fn visual_to_logical(visual_pos: usize, rendered_lines: &[String], inner_width: usize) -> Option<(usize, usize)> {
    use unicode_width::UnicodeWidthStr;

    if rendered_lines.is_empty() {
        return None;
    }
    if inner_width == 0 {
        // Degenerate case: treat each logical line as one visual line
        let row = visual_pos.min(rendered_lines.len() - 1);
        return Some((row, 0));
    }

    let mut cumulative_visual: usize = 0;
    for (logical, line) in rendered_lines.iter().enumerate() {
        let display_width = UnicodeWidthStr::width(line.as_str());
        let line_visual = if display_width == 0 {
            1
        } else {
            display_width.div_ceil(inner_width)
        };

        if cumulative_visual + line_visual > visual_pos {
            // The target visual line is within this logical line
            let offset_in_line = visual_pos - cumulative_visual;
            let col_offset = offset_in_line * inner_width;
            return Some((logical, col_offset));
        }
        cumulative_visual += line_visual;
    }

    // Past the end — return the last line
    Some((rendered_lines.len() - 1, 0))
}

/// Convert a screen coordinate to a text position in the message area.
///
/// `area` is the messages widget Rect (with borders), `scroll_offset` is the
/// current scroll in **visual** (wrapped) lines, and `rendered_lines` are the
/// logical plain-text lines used for selection extraction.
///
/// Returns None if the click is outside the content area (on borders).
pub fn screen_to_text_pos(
    screen_col: u16,
    screen_row: u16,
    area: Rect,
    scroll_offset: usize,
    rendered_lines: &[String],
) -> Option<TextPos> {
    // Check if click is within the inner area (inside borders)
    let inner_x = area.x + 1;
    let inner_y = area.y + 1;
    let inner_w = area.width.saturating_sub(2) as usize;
    let inner_h = area.height.saturating_sub(2);

    if screen_col < inner_x
        || screen_col >= inner_x + inner_w as u16
        || screen_row < inner_y
        || screen_row >= inner_y + inner_h
    {
        return None;
    }

    let col = (screen_col - inner_x) as usize;
    let visual_row = (screen_row - inner_y) as usize;
    let visual_pos = scroll_offset + visual_row;

    // Convert visual position to logical line + column offset
    let (logical_line, col_offset) = visual_to_logical(visual_pos, rendered_lines, inner_w)?;

    Some(TextPos::new(logical_line, col_offset + col))
}

/// Copy text to the system clipboard using the OSC 52 escape sequence.
/// This works in most modern terminals (iTerm2, kitty, alacritty, Windows Terminal, etc.)
pub fn copy_to_clipboard(text: &str) {
    if text.is_empty() {
        return;
    }
    let encoded = base64_encode(text);
    // OSC 52: Set clipboard. 'c' = system clipboard.
    let osc = format!("\x1b]52;c;{}\x07", encoded);
    let _ = std::io::stdout().write_all(osc.as_bytes());
    let _ = std::io::stdout().flush();
}

/// Simple base64 encoder (no external dep needed, we already have base64 crate)
fn base64_encode(input: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(input.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_pos_ordering() {
        let a = TextPos::new(0, 5);
        let b = TextPos::new(1, 0);
        let c = TextPos::new(0, 10);
        assert!(a < b);
        assert!(a < c);
        assert!(b > c);
    }

    #[test]
    fn test_selection_ordered() {
        // Drag from bottom to top
        let sel = TextSelection {
            anchor: TextPos::new(5, 10),
            cursor: TextPos::new(2, 3),
            active: false,
        };
        let (start, end) = sel.ordered();
        assert_eq!(start, TextPos::new(2, 3));
        assert_eq!(end, TextPos::new(5, 10));
    }

    #[test]
    fn test_extract_single_line() {
        let lines = vec!["Hello, world!".to_string()];
        let sel = TextSelection {
            anchor: TextPos::new(0, 7),
            cursor: TextPos::new(0, 12),
            active: false,
        };
        assert_eq!(sel.extract_text(&lines), "world");
    }

    #[test]
    fn test_extract_multi_line() {
        let lines = vec![
            "First line".to_string(),
            "Second line".to_string(),
            "Third line".to_string(),
        ];
        let sel = TextSelection {
            anchor: TextPos::new(0, 6),
            cursor: TextPos::new(2, 5),
            active: false,
        };
        assert_eq!(sel.extract_text(&lines), "line\nSecond line\nThird");
    }

    #[test]
    fn test_extract_empty_selection() {
        let lines = vec!["Hello".to_string()];
        let sel = TextSelection {
            anchor: TextPos::new(0, 3),
            cursor: TextPos::new(0, 3),
            active: false,
        };
        assert_eq!(sel.extract_text(&lines), "");
    }

    #[test]
    fn test_screen_to_text_pos_no_wrapping() {
        let area = Rect::new(0, 0, 80, 24);
        // 20 short lines that don't wrap (inner_width = 78)
        let lines: Vec<String> = (0..20).map(|i| format!("line {}", i)).collect();

        // Inside content area (1,1) to (78,22), scroll=10
        // visual_row=2 => visual_pos=12 => logical line 12
        let pos = screen_to_text_pos(5, 3, area, 10, &lines);
        assert_eq!(pos, Some(TextPos::new(12, 4)));

        // On border
        let pos = screen_to_text_pos(0, 0, area, 0, &lines);
        assert_eq!(pos, None);
    }

    #[test]
    fn test_screen_to_text_pos_with_wrapping() {
        let area = Rect::new(0, 0, 22, 24); // inner_width = 20
        // Line 0: 20 chars (1 visual line)
        // Line 1: 40 chars (2 visual lines)
        // Line 2: 20 chars (1 visual line)
        let lines = vec![
            "a]".repeat(10), // 20 display-width chars
            "b]".repeat(20), // 40 display-width chars → wraps to 2 visual lines
            "c]".repeat(10), // 20 display-width chars
        ];

        // No scroll, click visual row 0 → logical line 0
        let pos = screen_to_text_pos(5, 1, area, 0, &lines);
        assert_eq!(pos, Some(TextPos::new(0, 4)));

        // Click visual row 1 → logical line 1 (first row of wrapped line)
        let pos = screen_to_text_pos(5, 2, area, 0, &lines);
        assert_eq!(pos, Some(TextPos::new(1, 4)));

        // Click visual row 2 → still logical line 1 (second row of wrapped line)
        // col_offset = 20 (one inner_width of wrapped chars) + 4 = 24
        let pos = screen_to_text_pos(5, 3, area, 0, &lines);
        assert_eq!(pos, Some(TextPos::new(1, 24)));

        // Click visual row 3 → logical line 2
        let pos = screen_to_text_pos(5, 4, area, 0, &lines);
        assert_eq!(pos, Some(TextPos::new(2, 4)));
    }

    #[test]
    fn test_visual_to_logical_basic() {
        // 3 short lines, no wrapping at width 80
        let lines = vec!["hello".to_string(), "world".to_string(), "test".to_string()];
        assert_eq!(visual_to_logical(0, &lines, 80), Some((0, 0)));
        assert_eq!(visual_to_logical(1, &lines, 80), Some((1, 0)));
        assert_eq!(visual_to_logical(2, &lines, 80), Some((2, 0)));
        // Past the end → last line
        assert_eq!(visual_to_logical(5, &lines, 80), Some((2, 0)));
    }

    #[test]
    fn test_visual_to_logical_with_wrapping() {
        // Line 0: 10 chars (1 visual line at width 20)
        // Line 1: 50 chars (3 visual lines at width 20)
        // Line 2: 10 chars (1 visual line at width 20)
        let lines = vec!["a".repeat(10), "b".repeat(50), "c".repeat(10)];
        // visual 0 → logical 0
        assert_eq!(visual_to_logical(0, &lines, 20), Some((0, 0)));
        // visual 1 → logical 1, first wrap row
        assert_eq!(visual_to_logical(1, &lines, 20), Some((1, 0)));
        // visual 2 → logical 1, second wrap row (col_offset = 20)
        assert_eq!(visual_to_logical(2, &lines, 20), Some((1, 20)));
        // visual 3 → logical 1, third wrap row (col_offset = 40)
        assert_eq!(visual_to_logical(3, &lines, 20), Some((1, 40)));
        // visual 4 → logical 2
        assert_eq!(visual_to_logical(4, &lines, 20), Some((2, 0)));
    }

    #[test]
    fn test_col_range_for_row() {
        let sel = TextSelection {
            anchor: TextPos::new(1, 5),
            cursor: TextPos::new(3, 8),
            active: false,
        };
        // Row before selection
        assert_eq!(sel.col_range_for_row(0, 20), None);
        // First row of selection
        assert_eq!(sel.col_range_for_row(1, 20), Some((5, 20)));
        // Middle row - full line
        assert_eq!(sel.col_range_for_row(2, 15), Some((0, 15)));
        // Last row of selection
        assert_eq!(sel.col_range_for_row(3, 20), Some((0, 8)));
        // Row after selection
        assert_eq!(sel.col_range_for_row(4, 20), None);
    }
}
