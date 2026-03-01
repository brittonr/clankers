//! `@file` auto-read — expand `@path` references in prompts
//!
//! When a user types `@path/to/file.rs` in their prompt, this module
//! detects the reference, reads the file, and injects its contents inline.
//!
//! Supported patterns:
//! - `@path/to/file.rs` — Read entire file
//! - `@path/to/file.rs:10-20` — Read lines 10-20
//! - `@path/to/dir/` — List directory contents
//! - `@https://...` — Fetch URL (delegated to web tool)

use std::cmp::Reverse;
use std::path::Path;

/// A detected @file reference in the prompt text
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtFileRef {
    /// The full matched text (e.g., "@src/main.rs:10-20")
    pub raw: String,
    /// The path portion
    pub path: String,
    /// Optional line range
    pub line_range: Option<(usize, usize)>,
    /// Start position in the original text
    pub start: usize,
    /// End position in the original text
    pub end: usize,
}

/// Find all @file references in a prompt string
pub fn find_at_refs(text: &str) -> Vec<AtFileRef> {
    let mut refs = Vec::new();

    // Simple state-machine approach (avoids lookbehind):
    // Walk through the text character by character, looking for `@` preceded
    // by whitespace or start-of-string.
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '@' {
            // Check that @ is at the start or preceded by whitespace
            if i > 0 && !chars[i - 1].is_whitespace() {
                i += 1;
                continue;
            }

            // Collect the path characters after @
            let start = i;
            i += 1; // skip @
            let path_start = i;

            // Consume path characters: alphanumeric, _, ., /, -
            while i < len && (chars[i].is_alphanumeric() || "_./-:".contains(chars[i])) {
                i += 1;
            }

            if i == path_start {
                continue; // No path after @
            }

            let candidate: String = chars[path_start..i].iter().collect();

            // Must contain / or a file extension (.) to be a file reference
            // This avoids matching @mentions like @user
            if !candidate.contains('/') && !candidate.contains('.') {
                continue;
            }

            // Parse line range if present (path:10-20)
            let (path, line_range) = if let Some(colon_pos) = candidate.find(':') {
                let path_part = &candidate[..colon_pos];
                let range_part = &candidate[colon_pos + 1..];
                let range = parse_line_range(range_part);
                (path_part.to_string(), range)
            } else {
                (candidate.clone(), None)
            };

            let raw = format!("@{}", candidate);
            refs.push(AtFileRef {
                raw,
                path,
                line_range,
                start,
                end: i,
            });
        } else {
            i += 1;
        }
    }

    refs
}

/// Parse a line range like "10-20" or "42"
fn parse_line_range(s: &str) -> Option<(usize, usize)> {
    if let Some((start, end)) = s.split_once('-') {
        let start: usize = start.parse().ok()?;
        let end: usize = end.parse().ok()?;
        Some((start, end))
    } else {
        let line: usize = s.parse().ok()?;
        Some((line, line))
    }
}

/// Expand @file references in a prompt, replacing them with file contents.
/// Returns the expanded prompt text.
pub fn expand_at_refs(text: &str, cwd: &str) -> String {
    let refs = find_at_refs(text);
    if refs.is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();
    // Process in reverse order so indices stay valid
    let mut sorted_refs = refs;
    sorted_refs.sort_by_key(|r| Reverse(r.start));

    for at_ref in sorted_refs {
        let resolved = resolve_path(&at_ref.path, cwd);
        let content = read_file_content(&resolved, at_ref.line_range);
        let replacement = format_replacement(&at_ref.path, &content);

        // Replace the @ref with the file content
        // Find the exact position of this @ref in the current text
        if let Some(pos) = result.find(&at_ref.raw) {
            result.replace_range(pos..pos + at_ref.raw.len(), &replacement);
        }
    }

    result
}

/// Get completion suggestions for a partial @path
pub fn complete_at_path(partial: &str, cwd: &str) -> Vec<String> {
    let partial = partial.strip_prefix('@').unwrap_or(partial);
    let resolved = resolve_path(partial, cwd);

    let parent = if resolved.is_dir() {
        resolved.clone()
    } else {
        resolved.parent().unwrap_or(Path::new(".")).to_path_buf()
    };

    let prefix = if resolved.is_dir() {
        String::new()
    } else {
        resolved.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
    };

    let mut completions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&parent) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) {
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                let path = if partial.contains('/') {
                    let dir_part = &partial[..partial.rfind('/').unwrap_or(0) + 1];
                    format!("@{}{}{}", dir_part, name, if is_dir { "/" } else { "" })
                } else {
                    format!("@{}{}", name, if is_dir { "/" } else { "" })
                };
                completions.push(path);
            }
        }
    }

    completions.sort();
    completions.truncate(20);
    completions
}

fn resolve_path(path: &str, cwd: &str) -> std::path::PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        Path::new(cwd).join(p)
    }
}

fn read_file_content(path: &Path, line_range: Option<(usize, usize)>) -> String {
    if path.is_dir() {
        // List directory contents
        match std::fs::read_dir(path) {
            Ok(entries) => {
                let mut items: Vec<String> = entries
                    .flatten()
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        if is_dir { format!("{}/", name) } else { name }
                    })
                    .collect();
                items.sort();
                items.join("\n")
            }
            Err(e) => format!("[Error listing directory: {}]", e),
        }
    } else {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                if let Some((start, end)) = line_range {
                    let lines: Vec<&str> = content.lines().collect();
                    let start = start.saturating_sub(1); // Convert to 0-indexed
                    let end = end.min(lines.len());
                    lines[start..end].join("\n")
                } else {
                    // Limit to 500 lines to avoid blowing context
                    let lines: Vec<&str> = content.lines().collect();
                    if lines.len() > 500 {
                        let truncated: String = lines[..500].join("\n");
                        format!("{}\n\n[... {} more lines truncated]", truncated, lines.len() - 500)
                    } else {
                        content
                    }
                }
            }
            Err(e) => format!("[Error reading file: {}]", e),
        }
    }
}

fn format_replacement(path: &str, content: &str) -> String {
    // Determine language for syntax highlighting
    let lang = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("");

    format!("\n<file path=\"{}\">\n```{}\n{}\n```\n</file>\n", path, lang, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_at_refs_simple() {
        let text = "Look at @src/main.rs for details";
        let refs = find_at_refs(text);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].path, "src/main.rs");
        assert!(refs[0].line_range.is_none());
    }

    #[test]
    fn test_find_at_refs_with_line_range() {
        let text = "Check @src/lib.rs:10-20 ";
        let refs = find_at_refs(text);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].path, "src/lib.rs");
        assert_eq!(refs[0].line_range, Some((10, 20)));
    }

    #[test]
    fn test_find_at_refs_directory() {
        let text = "List @src/ contents";
        let refs = find_at_refs(text);
        assert_eq!(refs.len(), 1);
        assert!(refs[0].path.ends_with("src/"));
    }

    #[test]
    fn test_no_refs() {
        let text = "Just a normal message with email user@domain.com";
        let refs = find_at_refs(text);
        // Should not match email addresses
        assert!(refs.is_empty() || refs.iter().all(|r| r.path.contains('/')));
    }

    #[test]
    fn test_parse_line_range() {
        assert_eq!(parse_line_range("10-20"), Some((10, 20)));
        assert_eq!(parse_line_range("42"), Some((42, 42)));
        assert_eq!(parse_line_range("abc"), None);
    }

    #[test]
    fn test_format_replacement() {
        let result = format_replacement("src/main.rs", "fn main() {}");
        assert!(result.contains("```rs"));
        assert!(result.contains("fn main()"));
    }
}
