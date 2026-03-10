//! Output truncation utilities

use std::path::PathBuf;

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
pub fn truncate_head(content: &str, max_lines: usize, max_bytes: usize) -> (String, Option<PathBuf>) {
    let lines: Vec<&str> = content.lines().collect();

    // Check if we need to truncate by line count
    let needs_line_truncation = lines.len() > max_lines;

    // Check if we need to truncate by byte count
    let needs_byte_truncation = content.len() > max_bytes;

    if !needs_line_truncation && !needs_byte_truncation {
        return (content.to_string(), None);
    }

    let mut result = String::new();
    let mut current_bytes = 0;

    for (line_count, line) in lines.iter().enumerate() {
        if line_count >= max_lines {
            break;
        }

        let line_with_newline = format!("{}\n", line);
        let line_bytes = line_with_newline.len();

        if current_bytes + line_bytes > max_bytes {
            break;
        }

        result.push_str(&line_with_newline);
        current_bytes += line_bytes;
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
pub fn truncate_tail(content: &str, max_lines: usize, max_bytes: usize) -> (String, Option<PathBuf>) {
    let lines: Vec<&str> = content.lines().collect();

    // Check if we need to truncate
    let needs_line_truncation = lines.len() > max_lines;
    let needs_byte_truncation = content.len() > max_bytes;

    if !needs_line_truncation && !needs_byte_truncation {
        return (content.to_string(), None);
    }

    let mut result_lines = Vec::new();
    let mut current_bytes = 0;

    // Iterate from the end
    for line in lines.iter().rev() {
        if result_lines.len() >= max_lines {
            break;
        }

        let line_with_newline = format!("{}\n", line);
        let line_bytes = line_with_newline.len();

        if current_bytes + line_bytes > max_bytes {
            break;
        }

        result_lines.push(*line);
        current_bytes += line_bytes;
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
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_millis();
    let file_name = format!("clankers-output-{}.txt", timestamp);
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
        let (result, path) = truncate_tail("hello\nworld", 100, 100_000);
        assert_eq!(result, "hello\nworld");
        assert!(path.is_none());
    }

    #[test]
    fn test_truncate_by_lines() {
        let text = (0..100).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        let (result, _path) = truncate_tail(&text, 10, 100_000);
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines.len() <= 11); // 10 lines + possible truncation notice
    }

    #[test]
    fn test_truncate_by_bytes() {
        let text = "a".repeat(200);
        let (result, _path) = truncate_tail(&text, 1000, 50);
        assert!(result.len() <= 100); // 50 bytes + truncation notice
    }

    #[test]
    fn test_truncate_head_no_truncation() {
        let content = "line1\nline2\nline3";
        let (result, full_path) = truncate_head(content, 10, 1000);
        assert_eq!(result, content);
        assert!(full_path.is_none());
    }

    #[test]
    fn test_truncate_head_by_lines() {
        let content = "line1\nline2\nline3\nline4\nline5";
        let (result, full_path) = truncate_head(content, 3, 1000);
        assert_eq!(result, "line1\nline2\nline3");
        assert!(full_path.is_some());
    }

    #[test]
    fn test_truncate_head_by_bytes() {
        let content = "line1\nline2\nline3";
        let (result, full_path) = truncate_head(content, 100, 10);
        assert!(full_path.is_some());
        assert!(result.len() <= 10);
    }

    #[test]
    fn test_truncate_tail_no_truncation() {
        let content = "line1\nline2\nline3";
        let (result, full_path) = truncate_tail(content, 10, 1000);
        assert_eq!(result, content);
        assert!(full_path.is_none());
    }

    #[test]
    fn test_truncate_tail_by_lines() {
        let content = "line1\nline2\nline3\nline4\nline5";
        let (result, full_path) = truncate_tail(content, 3, 1000);
        assert_eq!(result, "line3\nline4\nline5");
        assert!(full_path.is_some());
    }
}
