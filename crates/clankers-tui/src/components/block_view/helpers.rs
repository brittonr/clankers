//! Utility functions for block rendering

use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

/// Format a duration as human-readable elapsed time
pub fn format_elapsed(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else {
        format!("{}m{:02}s", secs / 60, secs % 60)
    }
}

/// Build a horizontal rule of `─` that fills the remaining width.
/// `used` is how many columns are already consumed by the prefix on this line.
pub fn hrule(width: usize, used: usize) -> String {
    let remaining = width.saturating_sub(used);
    "─".repeat(remaining)
}

/// Build a horizontal rule of `┄` that fills the remaining width.
pub fn hrule_dotted(width: usize, used: usize) -> String {
    let remaining = width.saturating_sub(used);
    "┄".repeat(remaining)
}

/// Convert a column index (character offset) to a byte offset in a string.
pub fn char_to_byte(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map(|(i, _)| i).unwrap_or(s.len())
}

/// Slice a list of logical lines to only those visible in the current scroll
/// window, returning the sliced lines and a small residual scroll offset.
///
/// This works around ratatui's `u16` scroll limit (max 65,535 visual lines).
/// Instead of passing all lines with a potentially huge offset, we skip logical
/// lines that are entirely above the viewport and only pass a small residual
/// offset for the first partially-visible logical line.
///
/// Returns `(visible_lines, residual_offset)` where `residual_offset` is always
/// small enough to fit in a `u16`.
pub fn slice_visible_window<'a>(
    lines: &[Line<'a>],
    scroll_offset: usize,
    visible_height: usize,
    inner_width: usize,
) -> (Vec<Line<'a>>, usize) {
    if inner_width == 0 || lines.is_empty() {
        return (lines.to_vec(), scroll_offset.min(u16::MAX as usize));
    }

    // Find the first logical line whose visual lines overlap with the viewport.
    let mut visual_pos: usize = 0;
    let mut first_logical = 0;
    let mut residual: usize = 0;

    for (i, line) in lines.iter().enumerate() {
        let display_width: usize = line.spans.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum();
        let line_visual = if display_width == 0 {
            1
        } else {
            display_width.div_ceil(inner_width)
        };

        if visual_pos + line_visual > scroll_offset {
            // This logical line contains the scroll target
            first_logical = i;
            residual = scroll_offset - visual_pos;
            break;
        }
        visual_pos += line_visual;

        // If we've exhausted all lines without reaching the offset,
        // show the last line
        if i == lines.len() - 1 {
            first_logical = i;
            residual = 0;
        }
    }

    // Take enough logical lines to fill the viewport (with some buffer for
    // wrapped lines). We need at least `visible_height` visual lines past
    // the residual.
    let needed_visual = visible_height + residual;
    let mut collected_visual: usize = 0;
    let mut last_logical = first_logical;

    for line in &lines[first_logical..] {
        let display_width: usize = line.spans.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum();
        let line_visual = if display_width == 0 {
            1
        } else {
            display_width.div_ceil(inner_width)
        };
        collected_visual += line_visual;
        last_logical += 1;
        if collected_visual >= needed_visual {
            break;
        }
    }

    let sliced = lines[first_logical..last_logical].to_vec();
    (sliced, residual)
}
