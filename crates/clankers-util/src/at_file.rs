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
use std::process::Command;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use clanker_message::Content;
use clanker_message::ImageSource;
use serde::Deserialize;
use serde::Serialize;

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

/// Kind of context reference that was resolved or rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextReferenceKind {
    File,
    Directory,
    Image,
    GitDiff,
    Url,
    Unsupported,
    Error,
}

/// Resolution status for a context reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextReferenceStatus {
    Expanded,
    Unsupported,
    Error,
}

/// Safe metadata for one context-reference expansion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextReferenceMetadata {
    pub source: String,
    pub raw: String,
    pub kind: ContextReferenceKind,
    pub status: ContextReferenceStatus,
    pub target: String,
    pub line_range: Option<(usize, usize)>,
    pub line_count: Option<usize>,
    pub byte_count: Option<usize>,
    pub message: Option<String>,
}

impl ContextReferenceMetadata {
    fn expanded(
        at_ref: &AtFileRef,
        kind: ContextReferenceKind,
        target: String,
        line_count: Option<usize>,
        byte_count: Option<usize>,
    ) -> Self {
        Self {
            source: "context_references".to_string(),
            raw: sanitize_reference_raw(&at_ref.raw),
            kind,
            status: ContextReferenceStatus::Expanded,
            target,
            line_range: at_ref.line_range,
            line_count,
            byte_count,
            message: None,
        }
    }

    fn unsupported(at_ref: &AtFileRef, message: impl Into<String>) -> Self {
        Self {
            source: "context_references".to_string(),
            raw: sanitize_reference_raw(&at_ref.raw),
            kind: ContextReferenceKind::Unsupported,
            status: ContextReferenceStatus::Unsupported,
            target: unsupported_target(&at_ref.path),
            line_range: at_ref.line_range,
            line_count: None,
            byte_count: None,
            message: Some(message.into()),
        }
    }

    fn error(at_ref: &AtFileRef, target: String, message: impl Into<String>) -> Self {
        Self {
            source: "context_references".to_string(),
            raw: sanitize_reference_raw(&at_ref.raw),
            kind: ContextReferenceKind::Error,
            status: ContextReferenceStatus::Error,
            target,
            line_range: at_ref.line_range,
            line_count: None,
            byte_count: None,
            message: Some(message.into()),
        }
    }
}

/// Policy limits for context-reference expansion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextReferencePolicy {
    pub max_reference_bytes: usize,
    pub allow_url_fetch: bool,
    pub url_timeout_ms: u64,
}

impl Default for ContextReferencePolicy {
    fn default() -> Self {
        Self {
            max_reference_bytes: 64 * 1024,
            allow_url_fetch: false,
            url_timeout_ms: 2_000,
        }
    }
}

/// Result of expanding `@file` references — text plus any image content blocks
#[derive(Debug, Clone)]
pub struct ExpandedContent {
    /// The prompt text with `@refs` replaced (text files inlined, image refs replaced with labels)
    pub text: String,
    /// Image content blocks extracted from `@ref`'d image files
    pub images: Vec<Content>,
    /// Safe metadata for each context reference encountered.
    pub references: Vec<ContextReferenceMetadata>,
}

/// Find all @file references in a prompt string
pub fn find_at_refs(text: &str) -> Vec<AtFileRef> {
    let at_count = text.matches('@').count();
    assert!(text.chars().count() <= text.len());
    let mut refs = Vec::with_capacity(at_count);

    // Simple state-machine approach (avoids lookbehind):
    // Walk through the text character by character, looking for `@` preceded
    // by whitespace or start-of-string.
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '@' {
            // Check that @ is at the start or preceded by whitespace
            if i > 0 && !chars[i.saturating_sub(1)].is_whitespace() {
                i = i.saturating_add(1);
                continue;
            }

            // Collect the path characters after @
            let start = i;
            i = i.saturating_add(1); // skip @
            let path_start = i;

            // Consume path characters: alphanumeric, _, ., /, -
            while i < len && (chars[i].is_alphanumeric() || "_./-:@".contains(chars[i])) {
                i = i.saturating_add(1);
            }

            if i == path_start {
                continue; // No path after @
            }

            let candidate: String = chars[path_start..i].iter().collect();

            // Must be a local file-ish reference or one of the documented
            // non-file context reference prefixes. This avoids matching
            // ordinary @mentions while still accepting @diff.
            if !is_context_reference_candidate(&candidate) {
                continue;
            }

            // Parse line range if present (path:10-20). Only treat a colon as a
            // range separator when the suffix is a valid range so URL-like
            // references such as https://example.com stay intact for explicit
            // unsupported-reference handling.
            let (path, line_range) = if let Some(colon_pos) = candidate.rfind(':') {
                let path_part = &candidate[..colon_pos];
                let range_part = &candidate[colon_pos.saturating_add(1)..];
                if let Some(range) = parse_line_range(range_part) {
                    (path_part.to_string(), Some(range))
                } else {
                    (candidate.clone(), None)
                }
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

    assert!(refs.len() <= at_count);
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
    expand_at_refs_with_policy(text, cwd, &ContextReferencePolicy::default())
}

/// Expand context references with explicit policy controls for bounded diffs and URL fetches.
pub fn expand_at_refs_with_policy(text: &str, cwd: &str, policy: &ContextReferencePolicy) -> ExpandedContent {
    assert!(text.chars().count() <= text.len());
    assert!(cwd.chars().count() <= cwd.len());
    let refs = find_at_refs(text);
    if refs.is_empty() {
        return ExpandedContent {
            text: text.to_string(),
            images: Vec::new(),
            references: Vec::new(),
        };
    }

    let mut result = text.to_string();

    // Process in reverse order so indices stay valid
    let mut sorted_refs = refs;
    let mut images = Vec::with_capacity(sorted_refs.len());
    let mut references = Vec::with_capacity(sorted_refs.len());
    sorted_refs.sort_by_key(|r| Reverse(r.start));
    let reference_count = sorted_refs.len();

    for at_ref in sorted_refs {
        if is_git_diff_reference(&at_ref.path) {
            let diff = read_git_diff_reference(&at_ref.path, cwd, policy.max_reference_bytes);
            let replacement = format_replacement(&at_ref.raw, &diff.content);
            replace_raw(&mut result, &at_ref, &replacement);
            references.push(match diff.error {
                Some(message) => ContextReferenceMetadata::error(&at_ref, diff.target, message),
                None => ContextReferenceMetadata::expanded(
                    &at_ref,
                    ContextReferenceKind::GitDiff,
                    diff.target,
                    diff.line_count,
                    diff.byte_count,
                ),
            });
            continue;
        }

        if is_url_reference(&at_ref.path) {
            let fetched = read_url_reference(&at_ref.path, policy);
            let replacement = if fetched.error.is_some() && !policy.allow_url_fetch {
                format!(
                    "[Unsupported context reference {}: URL references are disabled by policy]",
                    sanitize_reference_raw(&at_ref.raw)
                )
            } else {
                format_replacement(&sanitize_reference_raw(&at_ref.raw), &fetched.content)
            };
            replace_raw(&mut result, &at_ref, &replacement);
            references.push(match fetched.error {
                Some(message) if policy.allow_url_fetch => {
                    ContextReferenceMetadata::error(&at_ref, fetched.target, message)
                }
                Some(message) => ContextReferenceMetadata::unsupported(&at_ref, message),
                None => ContextReferenceMetadata::expanded(
                    &at_ref,
                    ContextReferenceKind::Url,
                    fetched.target,
                    fetched.line_count,
                    fetched.byte_count,
                ),
            });
            continue;
        }

        if is_unsupported_reference(&at_ref.path) {
            let message = unsupported_message(&at_ref.path);
            let replacement = format!("[Unsupported context reference {}: {}]", at_ref.raw, message);
            replace_raw(&mut result, &at_ref, &replacement);
            references.push(ContextReferenceMetadata::unsupported(&at_ref, message));
            continue;
        }

        let resolved = resolve_path(&at_ref.path, cwd);
        let target = display_target(&resolved);

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
                    replace_raw(&mut result, &at_ref, &label);
                    references.push(ContextReferenceMetadata::expanded(
                        &at_ref,
                        ContextReferenceKind::Image,
                        target.clone(),
                        None,
                        Some(bytes.len()),
                    ));
                }
                Err(e) => {
                    let message = format!("Error reading image: {}", e);
                    let replacement = format!("[Error reading image {}: {}]", at_ref.path, e);
                    replace_raw(&mut result, &at_ref, &replacement);
                    references.push(ContextReferenceMetadata::error(&at_ref, target.clone(), message));
                }
            }
        } else {
            // Existing text file handling
            let read = read_file_content(&resolved, at_ref.line_range);
            let replacement = format_replacement(&at_ref.path, &read.content);
            replace_raw(&mut result, &at_ref, &replacement);
            references.push(match read.error {
                Some(message) => ContextReferenceMetadata::error(&at_ref, target.clone(), message),
                None => ContextReferenceMetadata::expanded(
                    &at_ref,
                    read.kind,
                    target.clone(),
                    read.line_count,
                    read.byte_count,
                ),
            });
        }
    }

    references.sort_by_key(|m| sorted_position_key(text, &m.raw));
    assert!(images.len() <= references.len());
    assert_eq!(references.len(), reference_count);

    ExpandedContent {
        text: result,
        images,
        references,
    }
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

#[derive(Debug)]
struct ReadContent {
    content: String,
    kind: ContextReferenceKind,
    line_count: Option<usize>,
    byte_count: Option<usize>,
    target: String,
    error: Option<String>,
}

fn read_file_content(path: &Path, line_range: Option<(usize, usize)>) -> ReadContent {
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
                let content = items.join("\n");
                ReadContent {
                    byte_count: Some(content.len()),
                    line_count: Some(items.len()),
                    content,
                    kind: ContextReferenceKind::Directory,
                    target: String::new(),
                    error: None,
                }
            }
            Err(e) => {
                let message = format!("Error listing directory: {}", e);
                ReadContent {
                    content: format!("[{}]", message),
                    kind: ContextReferenceKind::Error,
                    line_count: None,
                    byte_count: None,
                    target: String::new(),
                    error: Some(message),
                }
            }
        }
    } else {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let selected = if let Some((start, end)) = line_range {
                    let lines: Vec<&str> = content.lines().collect();
                    let start = start.saturating_sub(1); // Convert to 0-indexed
                    let end = end.min(lines.len());
                    if start >= end {
                        String::new()
                    } else {
                        lines[start..end].join("\n")
                    }
                } else {
                    // Limit to 500 lines to avoid blowing context
                    let lines: Vec<&str> = content.lines().collect();
                    if lines.len() > 500 {
                        let truncated: String = lines[..500].join("\n");
                        format!("{}\n\n[... {} more lines truncated]", truncated, lines.len().saturating_sub(500))
                    } else {
                        content
                    }
                };
                ReadContent {
                    byte_count: Some(selected.len()),
                    line_count: Some(selected.lines().count()),
                    content: selected,
                    kind: ContextReferenceKind::File,
                    target: String::new(),
                    error: None,
                }
            }
            Err(e) => {
                let message = format!("Error reading file: {}", e);
                ReadContent {
                    content: format!("[{}]", message),
                    kind: ContextReferenceKind::Error,
                    line_count: None,
                    byte_count: None,
                    target: String::new(),
                    error: Some(message),
                }
            }
        }
    }
}

fn replace_raw(result: &mut String, at_ref: &AtFileRef, replacement: &str) {
    if let Some(pos) = result.find(&at_ref.raw) {
        result.replace_range(pos..pos.saturating_add(at_ref.raw.len()), replacement);
    }
}

fn display_target(path: &Path) -> String {
    path.display().to_string()
}

fn sorted_position_key(text: &str, raw: &str) -> (bool, usize) {
    match text.find(raw) {
        Some(position) => (false, position),
        None => (true, 0),
    }
}

fn is_context_reference_candidate(candidate: &str) -> bool {
    candidate.contains('/')
        || candidate.contains('.')
        || is_git_diff_reference(candidate)
        || candidate.starts_with("session:")
        || candidate.starts_with("artifact:")
}

fn is_url_reference(path: &str) -> bool {
    path.starts_with("http://") || path.starts_with("https://")
}

fn is_git_diff_reference(path: &str) -> bool {
    path == "diff" || path.starts_with("diff:") || path == "git:diff" || path.starts_with("git:diff:")
}

fn read_git_diff_reference(path: &str, cwd: &str, max_bytes: usize) -> ReadContent {
    assert!(path.chars().count() <= path.len());
    assert!(cwd.chars().count() <= cwd.len());
    let target = if path == "diff" || path == "git:diff" {
        "git:diff".to_string()
    } else {
        format!("git:{}", path)
    };
    let mut command = Command::new("git");
    command.current_dir(cwd).args(["diff", "--no-ext-diff", "--no-color"]);
    if path == "diff:staged" || path == "git:diff:staged" {
        command.arg("--cached");
    } else if let Some(scope) = path.strip_prefix("diff:").or_else(|| path.strip_prefix("git:diff:"))
        && !scope.is_empty()
        && scope != "unstaged"
    {
        command.arg("--").arg(scope);
    }
    match command.output() {
        Ok(output) if output.status.success() => bounded_content(
            String::from_utf8_lossy(&output.stdout).into_owned(),
            ContextReferenceKind::GitDiff,
            target,
            max_bytes,
            "git diff reference exceeded configured byte limit",
        ),
        Ok(output) => ReadContent {
            content: format!("[Error expanding git diff reference: git exited with status {}]", output.status),
            kind: ContextReferenceKind::Error,
            line_count: None,
            byte_count: None,
            target,
            error: Some("git diff reference failed".to_string()),
        },
        Err(error) => ReadContent {
            content: format!("[Error expanding git diff reference: {}]", error),
            kind: ContextReferenceKind::Error,
            line_count: None,
            byte_count: None,
            target,
            error: Some("git diff reference failed".to_string()),
        },
    }
}

fn read_url_reference(path: &str, policy: &ContextReferencePolicy) -> ReadContent {
    assert!(path.chars().count() <= path.len());
    assert_eq!(policy.url_timeout_ms, policy.url_timeout_ms.saturating_add(0));
    let target = unsupported_target(path);
    if !policy.allow_url_fetch {
        return ReadContent {
            content: "[Unsupported context reference: URL fetching is disabled by policy]".to_string(),
            kind: ContextReferenceKind::Unsupported,
            line_count: None,
            byte_count: None,
            target,
            error: Some("URL references are disabled by policy".to_string()),
        };
    }
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(policy.url_timeout_ms))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            return ReadContent {
                content: "[Error fetching URL reference: client setup failed]".to_string(),
                kind: ContextReferenceKind::Error,
                line_count: None,
                byte_count: None,
                target,
                error: Some("URL fetch failed".to_string()),
            };
        }
    };
    match client
        .get(path)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.text())
    {
        Ok(content) => bounded_content(
            content,
            ContextReferenceKind::Url,
            target,
            policy.max_reference_bytes,
            "URL reference exceeded configured byte limit",
        ),
        Err(_) => ReadContent {
            content: "[Error fetching URL reference]".to_string(),
            kind: ContextReferenceKind::Error,
            line_count: None,
            byte_count: None,
            target,
            error: Some("URL fetch failed".to_string()),
        },
    }
}

fn bounded_content(
    content: String,
    kind: ContextReferenceKind,
    target: String,
    max_bytes: usize,
    limit_message: &'static str,
) -> ReadContent {
    assert!(!limit_message.is_empty());
    assert!(content.chars().count() <= content.len());
    if content.len() > max_bytes {
        return ReadContent {
            content: format!("[{}: {} > {} bytes]", limit_message, content.len(), max_bytes),
            kind: ContextReferenceKind::Error,
            line_count: None,
            byte_count: Some(content.len()),
            target,
            error: Some(limit_message.to_string()),
        };
    }
    ReadContent {
        line_count: Some(content.lines().count()),
        byte_count: Some(content.len()),
        content,
        kind,
        target,
        error: None,
    }
}

fn sanitize_reference_raw(raw: &str) -> String {
    if let Some(scheme_pos) = raw.find("://") {
        let authority_start = scheme_pos.saturating_add(3);
        let authority_end = raw[authority_start..]
            .find('/')
            .map(|idx| authority_start.saturating_add(idx))
            .unwrap_or(raw.len());
        let authority = &raw[authority_start..authority_end];
        if let Some(at_pos) = authority.rfind('@') {
            let host = &authority[at_pos.saturating_add(1)..];
            return format!("{}://[redacted]@{}{}", &raw[..scheme_pos], host, &raw[authority_end..]);
        }
    }
    raw.to_string()
}

fn is_unsupported_reference(path: &str) -> bool {
    path.starts_with("http://")
        || path.starts_with("https://")
        || path.starts_with("session:")
        || path.starts_with("artifact:")
        || path.starts_with("git:")
        || path == "diff"
        || path.starts_with("diff:")
}

fn unsupported_target(path: &str) -> String {
    if let Some((scheme, _)) = path.split_once(':') {
        format!("{}:", scheme)
    } else {
        path.to_string()
    }
}

fn unsupported_message(path: &str) -> &'static str {
    if path.starts_with("http://") || path.starts_with("https://") {
        "URL references are not supported yet"
    } else if path.starts_with("session:") || path.starts_with("artifact:") {
        "session artifact references are not supported yet"
    } else if path.starts_with("git:") || path == "diff" || path.starts_with("diff:") {
        "git diff references are not supported yet"
    } else {
        "reference kind is not supported yet"
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

    #[test]
    fn test_expand_records_file_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes.rs");
        std::fs::write(&file, "one\ntwo\nthree\n").unwrap();

        let result = expand_at_refs_with_images("read @notes.rs:2-3", dir.path().to_str().unwrap());

        assert!(result.text.contains("two\nthree"));
        assert_eq!(result.references.len(), 1);
        let reference = &result.references[0];
        assert_eq!(reference.source, "context_references");
        assert_eq!(reference.raw, "@notes.rs:2-3");
        assert_eq!(reference.kind, ContextReferenceKind::File);
        assert_eq!(reference.status, ContextReferenceStatus::Expanded);
        assert_eq!(reference.line_range, Some((2, 3)));
        assert_eq!(reference.line_count, Some(2));
        assert_eq!(reference.message, None);
    }

    #[test]
    fn test_expand_records_directory_metadata() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();

        let result = expand_at_refs_with_images("list @./", dir.path().to_str().unwrap());

        assert!(result.text.contains("a.txt"));
        assert!(result.text.contains("nested/"));
        assert_eq!(result.references.len(), 1);
        assert_eq!(result.references[0].kind, ContextReferenceKind::Directory);
        assert_eq!(result.references[0].status, ContextReferenceStatus::Expanded);
        assert_eq!(result.references[0].line_count, Some(2));
    }

    #[test]
    fn test_unsupported_url_is_explicit() {
        let result = expand_at_refs_with_images("fetch @https://example.com/path", "/tmp");

        assert!(result.images.is_empty());
        assert!(result.text.contains("Unsupported context reference @https://example.com/path"));
        assert!(result.text.contains("URL references are disabled by policy"));
        assert_eq!(result.references.len(), 1);
        assert_eq!(result.references[0].raw, "@https://example.com/path");
        assert_eq!(result.references[0].kind, ContextReferenceKind::Unsupported);
        assert_eq!(result.references[0].status, ContextReferenceStatus::Unsupported);
        assert_eq!(result.references[0].target, "https:");
    }

    #[test]
    fn test_git_diff_reference_expands_bounded_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let status = Command::new("git").current_dir(dir.path()).args(["init"]).status().unwrap();
        assert!(status.success());
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "one\n").unwrap();
        Command::new("git").current_dir(dir.path()).args(["add", "notes.txt"]).status().unwrap();
        Command::new("git")
            .current_dir(dir.path())
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "init",
            ])
            .status()
            .unwrap();
        std::fs::write(&file, "one\ntwo\n").unwrap();

        let result = expand_at_refs_with_policy(
            "review @diff",
            dir.path().to_str().unwrap(),
            &ContextReferencePolicy::default(),
        );

        assert!(result.text.contains("+two"));
        assert_eq!(result.references.len(), 1);
        assert_eq!(result.references[0].kind, ContextReferenceKind::GitDiff);
        assert_eq!(result.references[0].status, ContextReferenceStatus::Expanded);
        assert_eq!(result.references[0].target, "git:diff");
        assert!(result.references[0].byte_count.unwrap_or_default() > 0);
    }

    #[test]
    fn test_url_reference_fetches_when_policy_allows() {
        use std::io::Read;
        use std::io::Write;
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0_u8; 512];
            let _ = stream.read(&mut buf);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\nConnection: close\r\n\r\nhello url ref")
                .unwrap();
        });
        let policy = ContextReferencePolicy {
            allow_url_fetch: true,
            ..ContextReferencePolicy::default()
        };
        let prompt = format!("read @http://{addr}/note");

        let result = expand_at_refs_with_policy(&prompt, "/tmp", &policy);
        handle.join().unwrap();

        assert!(result.text.contains("hello url ref"));
        assert_eq!(result.references.len(), 1);
        assert_eq!(result.references[0].kind, ContextReferenceKind::Url);
        assert_eq!(result.references[0].status, ContextReferenceStatus::Expanded);
        assert_eq!(result.references[0].target, "http:");
    }

    #[test]
    fn test_url_reference_metadata_redacts_userinfo() {
        let result = expand_at_refs_with_images("fetch @https://token@example.com/private", "/tmp");

        let rendered = serde_json::to_string(&result.references).unwrap();
        assert!(!rendered.contains("token@example.com"));
        assert!(rendered.contains("[redacted]@example.com"));
        assert!(result.text.contains("[redacted]@example.com"));
    }

    #[test]
    fn test_missing_file_records_error_metadata() {
        let result = expand_at_refs_with_images("look at @missing.rs", "/tmp");

        assert!(result.text.contains("Error reading file"));
        assert_eq!(result.references.len(), 1);
        assert_eq!(result.references[0].kind, ContextReferenceKind::Error);
        assert_eq!(result.references[0].status, ContextReferenceStatus::Error);
        assert!(result.references[0].message.as_deref().unwrap_or_default().contains("Error reading file"));
    }
}
