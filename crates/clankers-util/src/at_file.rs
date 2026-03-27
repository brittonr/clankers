//! `@file` auto-read — expand `@path` references in prompts
//!
//! When a user types `@path/to/file.rs` in their prompt, this module
//! detects the reference, reads the file, and injects its contents inline.
//!
//! Supported patterns:
//! - `@path/to/file.rs` — Read entire file
//! - `@path/to/file.rs:10-20` — Read lines 10-20
//! - `@path/to/dir/` — List directory contents
//! - `@path/to/image.png` — Attach image as base64 content block
//! - `@https://...` — Fetch URL (delegated to web tool)

use std::cmp::Reverse;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use clankers_message::message::Content;
use clankers_message::message::ImageSource;

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

/// Image file extensions that produce `Content::Image` blocks instead of inline text
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp"];

/// Check whether a path has an image extension
fn is_image_extension(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Map an image file extension to its MIME type
fn image_media_type(path: &str) -> String {
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Result of expanding `@file` references — text plus any image content blocks
#[derive(Debug, Clone)]
pub struct ExpandedContent {
    /// The prompt text with `@refs` replaced (text files inlined, image refs replaced with labels)
    pub text: String,
    /// Image content blocks extracted from `@ref`'d image files
    pub images: Vec<Content>,
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
/// Expand `@file` references, returning structured content with text + images.
///
/// Image files (`.jpg`, `.png`, `.gif`, `.webp`) are base64-encoded as
/// `Content::Image` blocks. Text files are inlined as before.
pub fn expand_at_refs_with_images(text: &str, cwd: &str) -> ExpandedContent {
    let refs = find_at_refs(text);
    if refs.is_empty() {
        return ExpandedContent {
            text: text.to_string(),
            images: Vec::new(),
        };
    }

    let mut result = text.to_string();
    let mut images = Vec::new();

    // Process in reverse order so indices stay valid
    let mut sorted_refs = refs;
    sorted_refs.sort_by_key(|r| Reverse(r.start));

    for at_ref in sorted_refs {
        let resolved = resolve_path(&at_ref.path, cwd);

        if is_image_extension(&at_ref.path) {
            // Read as binary image, encode to base64
            match std::fs::read(&resolved) {
                Ok(bytes) => {
                    let media_type = image_media_type(&at_ref.path);
                    let data = BASE64.encode(&bytes);
                    images.push(Content::Image {
                        source: ImageSource::Base64 { media_type, data },
                    });
                    // Replace the @ref with a short label in the text
                    let label = format!("[image: {}]", at_ref.path);
                    if let Some(pos) = result.find(&at_ref.raw) {
                        result.replace_range(pos..pos + at_ref.raw.len(), &label);
                    }
                }
                Err(e) => {
                    let replacement = format!("[Error reading image {}: {}]", at_ref.path, e);
                    if let Some(pos) = result.find(&at_ref.raw) {
                        result.replace_range(pos..pos + at_ref.raw.len(), &replacement);
                    }
                }
            }
        } else {
            // Existing text file handling
            let content = read_file_content(&resolved, at_ref.line_range);
            let replacement = format_replacement(&at_ref.path, &content);
            if let Some(pos) = result.find(&at_ref.raw) {
                result.replace_range(pos..pos + at_ref.raw.len(), &replacement);
            }
        }
    }

    ExpandedContent { text: result, images }
}

/// Get completion suggestions for a partial @path
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

    #[test]
    fn test_is_image_extension() {
        assert!(is_image_extension("photo.png"));
        assert!(is_image_extension("photo.PNG"));
        assert!(is_image_extension("photo.jpg"));
        assert!(is_image_extension("photo.jpeg"));
        assert!(is_image_extension("photo.gif"));
        assert!(is_image_extension("dir/photo.webp"));
        assert!(!is_image_extension("code.rs"));
        assert!(!is_image_extension("readme.md"));
        assert!(!is_image_extension("noext"));
    }

    #[test]
    fn test_image_media_type() {
        assert_eq!(image_media_type("test.png"), "image/png");
        assert_eq!(image_media_type("test.jpg"), "image/jpeg");
        assert_eq!(image_media_type("test.jpeg"), "image/jpeg");
        assert_eq!(image_media_type("test.gif"), "image/gif");
        assert_eq!(image_media_type("test.webp"), "image/webp");
        assert_eq!(image_media_type("test.bmp"), "application/octet-stream");
    }

    #[test]
    fn test_expand_at_refs_with_images_no_refs() {
        let result = expand_at_refs_with_images("just plain text", "/tmp");
        assert_eq!(result.text, "just plain text");
        assert!(result.images.is_empty());
    }

    #[test]
    fn test_expand_at_refs_with_images_text_file() {
        // Non-existent text file should produce an error in text, no images
        let result = expand_at_refs_with_images("look at @nonexistent.rs", "/tmp");
        assert!(result.images.is_empty());
        assert!(result.text.contains("Error reading file"));
    }

    #[test]
    fn test_expand_at_refs_with_images_missing_image() {
        let result = expand_at_refs_with_images("check @missing.png", "/tmp");
        assert!(result.images.is_empty());
        assert!(result.text.contains("Error reading image"));
    }

    #[test]
    fn test_expand_at_refs_with_images_real_image() {
        // Create a temp image file
        let dir = std::env::temp_dir();
        let img_path = dir.join("test_at_file.png");
        std::fs::write(&img_path, b"fake png bytes").ok();

        let text = format!("look at @{}", img_path.display());
        let result = expand_at_refs_with_images(&text, "/");
        assert_eq!(result.images.len(), 1);
        assert!(result.text.contains("[image:"));
        match &result.images[0] {
            Content::Image {
                source: ImageSource::Base64 { media_type, .. },
            } => {
                assert_eq!(media_type, "image/png");
            }
            other => panic!("Expected Content::Image, got {:?}", other),
        }

        std::fs::remove_file(&img_path).ok();
    }
}
