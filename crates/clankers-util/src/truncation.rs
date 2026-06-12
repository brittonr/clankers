//! Output truncation utilities

use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

static OUTPUT_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub struct TruncationRequest<'a> {
    pub content: &'a str,
    pub max_lines: usize,
    pub max_bytes: usize,
}

/// Truncate content from the head (keep first N lines/bytes).
///
/// Returns the truncated content and optionally a path to the full output file.
///
/// # Arguments
///
/// * `content` - The content to truncate
/// * `max_lines` - Maximum number of lines to keep
/// * `max_bytes` - Maximum number of bytes to keep
///
/// # Returns
///
/// A tuple of (truncated_content, full_output_path)
pub fn truncate_head(request: TruncationRequest<'_>) -> (String, Option<PathBuf>) {
    let content = request.content;
    let max_lines = request.max_lines;
    let max_bytes = request.max_bytes;
    assert!(content.chars().count() <= content.len());
    let lines: Vec<&str> = content.lines().collect();
    assert!(lines.len() <= content.len().saturating_add(1));

    // Check if we need to truncate by line count
    let needs_line_truncation = lines.len() > max_lines;

    // Check if we need to truncate by byte count
    let needs_byte_truncation = content.len() > max_bytes;

    if !needs_line_truncation && !needs_byte_truncation {
        return (content.to_string(), None);
    }

    let mut result = String::new();
    let mut current_bytes: usize = 0;

    for (line_count, line) in lines.iter().enumerate() {
        if line_count >= max_lines {
            break;
        }

        let line_with_newline = format!("{}\n", line);
        let line_bytes = line_with_newline.len();

        if current_bytes.saturating_add(line_bytes) > max_bytes {
            break;
        }

        result.push_str(&line_with_newline);
        current_bytes = current_bytes.saturating_add(line_bytes);
    }

    // Remove trailing newline if present
    if result.ends_with('\n') {
        result.pop();
    }

    // Save full output to temp file
    let full_path = save_full_output(content);

    (result, full_path)
}

/// Truncate content from the tail (keep last N lines/bytes).
///
/// Returns the truncated content and optionally a path to the full output file.
///
/// # Arguments
///
/// * `content` - The content to truncate
/// * `max_lines` - Maximum number of lines to keep
/// * `max_bytes` - Maximum number of bytes to keep
///
/// # Returns
///
/// A tuple of (truncated_content, full_output_path)
pub fn truncate_tail(request: TruncationRequest<'_>) -> (String, Option<PathBuf>) {
    let content = request.content;
    let max_lines = request.max_lines;
    let max_bytes = request.max_bytes;
    assert!(content.chars().count() <= content.len());
    let lines: Vec<&str> = content.lines().collect();
    assert!(lines.len() <= content.len().saturating_add(1));

    // Check if we need to truncate
    let needs_line_truncation = lines.len() > max_lines;
    let needs_byte_truncation = content.len() > max_bytes;

    if !needs_line_truncation && !needs_byte_truncation {
        return (content.to_string(), None);
    }

    let mut result_lines = Vec::new();
    let mut current_bytes: usize = 0;

    // Iterate from the end
    for line in lines.iter().rev() {
        if result_lines.len() >= max_lines {
            break;
        }

        let line_with_newline = format!("{}\n", line);
        let line_bytes = line_with_newline.len();

        if current_bytes.saturating_add(line_bytes) > max_bytes {
            break;
        }

        result_lines.push(*line);
        current_bytes = current_bytes.saturating_add(line_bytes);
    }

    // Reverse to get original order
    result_lines.reverse();

    let result = result_lines.join("\n");

    // Save full output to temp file
    let full_path = save_full_output(content);

    (result, full_path)
}

/// Save full output to a temporary file
fn save_full_output(content: &str) -> Option<PathBuf> {
    use std::io::Write;

    let temp_dir = std::env::temp_dir();
    let sequence = OUTPUT_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let process_id = std::process::id();
    let file_name = format!("clankers-output-{}-{}.txt", process_id, sequence);
    let path = temp_dir.join(file_name);

    let mut file = std::fs::File::create(&path).ok()?;
    file.write_all(content.as_bytes()).ok()?;

    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short_text() {
        let (result, path) = truncate_tail(TruncationRequest { content: "hello\nworld", max_lines: 100, max_bytes: 100_000 });
        assert_eq!(result, "hello\nworld");
        assert!(path.is_none());
    }

    #[test]
    fn test_truncate_by_lines() {
        let text = (0..100).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        let (result, _path) = truncate_tail(TruncationRequest { content: &text, max_lines: 10, max_bytes: 100_000 });
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines.len() <= 11); // 10 lines + possible truncation notice
    }

    #[test]
    fn test_truncate_by_bytes() {
        let text = "a".repeat(200);
        let (result, _path) = truncate_tail(TruncationRequest { content: &text, max_lines: 1000, max_bytes: 50 });
        assert!(result.len() <= 100); // 50 bytes + truncation notice
    }

    #[test]
    fn test_truncate_head_no_truncation() {
        let content = "line1\nline2\nline3";
        let (result, full_path) = truncate_head(TruncationRequest { content: content, max_lines: 10, max_bytes: 1000 });
        assert_eq!(result, content);
        assert!(full_path.is_none());
    }

    #[test]
    fn test_truncate_head_by_lines() {
        let content = "line1\nline2\nline3\nline4\nline5";
        let (result, full_path) = truncate_head(TruncationRequest { content: content, max_lines: 3, max_bytes: 1000 });
        assert_eq!(result, "line1\nline2\nline3");
        assert!(full_path.is_some());
    }

    #[test]
    fn test_truncate_head_by_bytes() {
        let content = "line1\nline2\nline3";
        let (result, full_path) = truncate_head(TruncationRequest { content: content, max_lines: 100, max_bytes: 10 });
        assert!(full_path.is_some());
        assert!(result.len() <= 10);
    }

    #[test]
    fn test_truncate_tail_no_truncation() {
        let content = "line1\nline2\nline3";
        let (result, full_path) = truncate_tail(TruncationRequest { content: content, max_lines: 10, max_bytes: 1000 });
        assert_eq!(result, content);
        assert!(full_path.is_none());
    }

    #[test]
    fn test_truncate_tail_by_lines() {
        let content = "line1\nline2\nline3\nline4\nline5";
        let (result, full_path) = truncate_tail(TruncationRequest { content: content, max_lines: 3, max_bytes: 1000 });
        assert_eq!(result, "line3\nline4\nline5");
        assert!(full_path.is_some());
    }
}
