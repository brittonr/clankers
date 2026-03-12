//! Tool output truncation — cap tool results before inserting into conversation.
//!
//! When output exceeds configured limits (line count or byte count), the full
//! output is saved to a temp file and the conversation receives a truncated
//! version with a reference path and `read` command.

use std::path::PathBuf;

/// Truncation limits for tool output.
#[derive(Debug, Clone)]
pub struct OutputTruncationConfig {
    /// Maximum bytes before truncation (default: 50KB).
    pub max_bytes: usize,
    /// Maximum lines before truncation (default: 2000).
    pub max_lines: usize,
    /// Whether truncation is enabled at all.
    pub enabled: bool,
}

impl Default for OutputTruncationConfig {
    fn default() -> Self {
        Self {
            max_bytes: 50 * 1024,
            max_lines: 2000,
            enabled: true,
        }
    }
}

/// Result of attempting truncation.
pub struct TruncationResult {
    /// The (possibly truncated) content to use in conversation.
    pub content: String,
    /// Whether truncation was applied.
    pub truncated: bool,
    /// Path to full output if truncated.
    pub full_output_path: Option<PathBuf>,
    /// Original line count.
    pub original_lines: usize,
    /// Original byte count.
    pub original_bytes: usize,
}

/// Truncate tool output if it exceeds configured limits.
///
/// Returns the original content unchanged if within limits or if truncation
/// is disabled. Otherwise saves full output to a temp file and returns
/// truncated content with a reference footer.
pub fn truncate_tool_output(content: &str, config: &OutputTruncationConfig) -> TruncationResult {
    let original_bytes = content.len();
    let lines: Vec<&str> = content.lines().collect();
    let original_lines = lines.len();

    if !config.enabled || (original_lines <= config.max_lines && original_bytes <= config.max_bytes) {
        return TruncationResult {
            content: content.to_string(),
            truncated: false,
            full_output_path: None,
            original_lines,
            original_bytes,
        };
    }

    // Determine how many lines to keep (whichever limit is hit first)
    let mut kept_bytes = 0;
    let mut kept_lines = 0;
    for line in &lines {
        let line_bytes = line.len() + 1; // +1 for newline
        if kept_lines >= config.max_lines || kept_bytes + line_bytes > config.max_bytes {
            break;
        }
        kept_bytes += line_bytes;
        kept_lines += 1;
    }

    // Edge case: if no complete line fits within byte limit, keep at least one
    // line truncated to max_bytes so the model gets something useful.
    if kept_lines == 0 && !lines.is_empty() {
        kept_lines = 1;
    }

    let tmp_path = save_full_output(content);

    let truncated_text = lines[..kept_lines].join("\n");
    let truncated_content = format!(
        "{}\n\n[Output truncated: {} lines / {} total. Full output saved to {}]\n\
         [Use `read {}` with offset/limit to see the rest]",
        truncated_text,
        original_lines,
        format_size(original_bytes),
        tmp_path.display(),
        tmp_path.display(),
    );

    TruncationResult {
        content: truncated_content,
        truncated: true,
        full_output_path: Some(tmp_path),
        original_lines,
        original_bytes,
    }
}

/// Clean up temp files older than the given duration.
pub fn cleanup_temp_files(max_age: std::time::Duration) {
    let dir = temp_output_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if let Ok(age) = now.duration_since(modified)
            && age > max_age
        {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn temp_output_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("clankers-tool-output");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn save_full_output(content: &str) -> PathBuf {
    let dir = temp_output_dir();
    let id = uuid::Uuid::new_v4().simple().to_string();
    let path = dir.join(format!("{}.txt", id));
    // Best-effort write; if it fails the truncated content still works
    let _ = std::fs::write(&path, content);
    path
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> OutputTruncationConfig {
        OutputTruncationConfig::default()
    }

    #[test]
    fn within_limits_unchanged() {
        let content = "line 1\nline 2\nline 3";
        let result = truncate_tool_output(content, &default_config());
        assert!(!result.truncated);
        assert_eq!(result.content, content);
        assert!(result.full_output_path.is_none());
        assert_eq!(result.original_lines, 3);
        assert_eq!(result.original_bytes, content.len());
    }

    #[test]
    fn exceeds_line_limit() {
        let config = OutputTruncationConfig {
            max_lines: 10,
            max_bytes: 1024 * 1024,
            enabled: true,
        };
        let lines: Vec<String> = (0..50).map(|i| format!("line {}", i)).collect();
        let content = lines.join("\n");

        let result = truncate_tool_output(&content, &config);
        assert!(result.truncated);
        assert_eq!(result.original_lines, 50);
        assert!(result.full_output_path.is_some());
        // Should contain first 10 lines
        assert!(result.content.contains("line 0"));
        assert!(result.content.contains("line 9"));
        // Should not contain line 10+
        assert!(!result.content.contains("\nline 10\n"));
        // Should contain footer
        assert!(result.content.contains("[Output truncated:"));
        assert!(result.content.contains("50 lines"));
    }

    #[test]
    fn exceeds_byte_limit() {
        let config = OutputTruncationConfig {
            max_lines: 100_000,
            max_bytes: 100,
            enabled: true,
        };
        // Each line is 20 chars + newline = 21 bytes. 100 bytes fits ~4 lines.
        let lines: Vec<String> = (0..20).map(|i| format!("long-line-content-{:02}", i)).collect();
        let content = lines.join("\n");

        let result = truncate_tool_output(&content, &config);
        assert!(result.truncated);
        assert!(result.full_output_path.is_some());
        assert!(result.content.contains("[Output truncated:"));
    }

    #[test]
    fn line_limit_hit_first() {
        let config = OutputTruncationConfig {
            max_lines: 5,
            max_bytes: 1024 * 1024,
            enabled: true,
        };
        // 100 short lines — line limit hit before byte limit
        let lines: Vec<String> = (0..100).map(|i| format!("L{}", i)).collect();
        let content = lines.join("\n");

        let result = truncate_tool_output(&content, &config);
        assert!(result.truncated);
        assert_eq!(result.original_lines, 100);
        // Only first 5 lines in output
        let output_before_footer = result.content.split("\n\n[Output truncated:").next().unwrap();
        assert_eq!(output_before_footer.lines().count(), 5);
    }

    #[test]
    fn byte_limit_hit_first() {
        let config = OutputTruncationConfig {
            max_lines: 100_000,
            max_bytes: 50,
            enabled: true,
        };
        // Lines that total well over 50 bytes
        let lines: Vec<String> = (0..10).map(|i| format!("this-is-a-long-line-{:03}", i)).collect();
        let content = lines.join("\n");
        assert!(content.len() > 50);

        let result = truncate_tool_output(&content, &config);
        assert!(result.truncated);
    }

    #[test]
    fn footer_contains_path() {
        let config = OutputTruncationConfig {
            max_lines: 2,
            max_bytes: 1024 * 1024,
            enabled: true,
        };
        let content = "a\nb\nc\nd\ne";
        let result = truncate_tool_output(content, &config);
        assert!(result.truncated);
        let path = result.full_output_path.as_ref().unwrap();
        assert!(result.content.contains(&path.display().to_string()));
    }

    #[test]
    fn footer_contains_read_command() {
        let config = OutputTruncationConfig {
            max_lines: 2,
            max_bytes: 1024 * 1024,
            enabled: true,
        };
        let content = "a\nb\nc\nd\ne";
        let result = truncate_tool_output(content, &config);
        assert!(result.content.contains("Use `read"));
        assert!(result.content.contains("with offset/limit"));
    }

    #[test]
    fn temp_file_contains_full_output() {
        let config = OutputTruncationConfig {
            max_lines: 2,
            max_bytes: 1024 * 1024,
            enabled: true,
        };
        let content = "line1\nline2\nline3\nline4\nline5";
        let result = truncate_tool_output(content, &config);
        assert!(result.truncated);
        let path = result.full_output_path.unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert_eq!(saved, content);
        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_content_unchanged() {
        let result = truncate_tool_output("", &default_config());
        assert!(!result.truncated);
        assert_eq!(result.content, "");
        assert_eq!(result.original_lines, 0);
        assert_eq!(result.original_bytes, 0);
    }

    #[test]
    fn single_long_line_over_byte_limit() {
        let config = OutputTruncationConfig {
            max_lines: 100,
            max_bytes: 50,
            enabled: true,
        };
        let content = "x".repeat(200);
        let result = truncate_tool_output(&content, &config);
        // Single line exceeds byte limit — should still keep 1 line
        assert!(result.truncated);
        assert!(result.full_output_path.is_some());
        assert!(result.content.contains("[Output truncated:"));
    }

    #[test]
    fn disabled_truncation_passes_through() {
        let config = OutputTruncationConfig {
            max_lines: 2,
            max_bytes: 10,
            enabled: false,
        };
        let content = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj";
        let result = truncate_tool_output(content, &config);
        assert!(!result.truncated);
        assert_eq!(result.content, content);
    }

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(500), "500B");
        assert_eq!(format_size(1024), "1.0KB");
        assert_eq!(format_size(51200), "50.0KB");
        assert_eq!(format_size(1048576), "1.0MB");
        assert_eq!(format_size(2621440), "2.5MB");
    }

    #[test]
    fn cleanup_removes_old_files() {
        let dir = temp_output_dir();
        let old_file = dir.join("old-test-file.txt");
        std::fs::write(&old_file, "old").unwrap();

        // Set modification time to 2 days ago
        let two_days_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(48 * 3600);
        filetime::set_file_mtime(&old_file, filetime::FileTime::from_system_time(two_days_ago)).ok();

        let recent_file = dir.join("recent-test-file.txt");
        std::fs::write(&recent_file, "recent").unwrap();

        cleanup_temp_files(std::time::Duration::from_secs(24 * 3600));

        // Old file should be removed, recent should remain
        assert!(!old_file.exists());
        assert!(recent_file.exists());

        // Cleanup
        let _ = std::fs::remove_file(&recent_file);
    }
}
