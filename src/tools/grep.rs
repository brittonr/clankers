//! In-process file content search using the `grep` and `ignore` crates.
//!
//! Uses `ignore::WalkBuilder` for .gitignore-respecting file traversal and
//! `grep-regex` + `grep-searcher` for high-performance regex matching.
//! No external `rg` binary required.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use grep_regex::RegexMatcher;
use grep_searcher::Searcher;
use grep_searcher::sinks::UTF8;
use ignore::WalkBuilder;
use serde_json::Value;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use super::progress::{ResultChunk, ToolProgress};

pub struct GrepTool {
    definition: ToolDefinition,
}

impl GrepTool {
    pub fn new() -> Self {
        let definition = ToolDefinition {
            name: "grep".to_string(),
            description: "Search file contents using regex. Returns matching lines with file \
                paths and line numbers. Respects .gitignore. Output is truncated to 2000 lines \
                or 50KB."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Search pattern (regex)"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory or file to search (default: current directory)"
                    },
                    "glob": {
                        "type": "string",
                        "description": "File glob pattern, e.g. '*.rs'"
                    },
                    "case_sensitive": {
                        "type": "boolean",
                        "description": "Case-sensitive search (default: smart case — sensitive if pattern contains uppercase)",
                        "default": null
                    }
                },
                "required": ["pattern"]
            }),
        };

        Self { definition }
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let pattern = match params.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolResult::error("Missing required parameter: pattern"),
        };

        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".").to_string();

        let glob = params.get("glob").and_then(|v| v.as_str()).map(String::from);

        let case_sensitive = params.get("case_sensitive").and_then(|v| v.as_bool());

        // Run the search in a blocking task to avoid blocking the async runtime
        let cancel = ctx.signal.clone();
        let progress_ctx = ctx.clone();
        let result = tokio::task::spawn_blocking(move || {
            search_files(&pattern, &path, glob.as_deref(), case_sensitive, &cancel, |msg| {
                progress_ctx.emit_progress(msg);
                // Extract match count from "path (N matches)" format for structured progress
                if let Some(start) = msg.rfind('(')
                    && let Some(end) = msg.rfind(" matches)")
                    && let Ok(count) = msg[start + 1..end].parse::<u64>()
                {
                    progress_ctx.emit_structured_progress(
                        ToolProgress::lines(count, None)
                            .with_message("Searching"),
                    );
                }
            })
        })
        .await;

        match result {
            Ok(Ok(output)) => {
                if output.is_empty() {
                    return ToolResult::text("No matches found");
                }

                // Emit the full output as a result chunk for the accumulator
                ctx.emit_result_chunk(ResultChunk::text(&output));

                // Apply truncation
                const MAX_LINES: usize = 2000;
                const MAX_BYTES: usize = 50 * 1024;

                let (truncated_output, full_output_path) =
                    crate::tools::truncation::truncate_tail(&output, MAX_LINES, MAX_BYTES);

                let mut result = ToolResult::text(truncated_output);
                if let Some(path) = full_output_path {
                    result.full_output_path = Some(path.display().to_string());
                }
                result
            }
            Ok(Err(e)) => {
                if e.contains("cancelled") {
                    ctx.emit_structured_progress(ToolProgress::phase("Cancelling", 1, Some(1)));
                }
                ToolResult::error(e)
            }
            Err(e) => ToolResult::error(format!("Search task panicked: {}", e)),
        }
    }
}

/// Perform the actual in-process search.
fn search_files(
    pattern: &str,
    path: &str,
    glob: Option<&str>,
    case_sensitive: Option<bool>,
    cancel: &CancellationToken,
    progress: impl Fn(&str),
) -> Result<String, String> {
    let mut builder = grep_regex::RegexMatcherBuilder::new();
    builder.line_terminator(Some(b'\n'));

    match case_sensitive {
        Some(true) => {
            builder.case_insensitive(false).case_smart(false);
        }
        Some(false) => {
            builder.case_insensitive(true).case_smart(false);
        }
        None => {
            // Smart case: case-insensitive unless pattern has uppercase
            builder.case_smart(true);
        }
    }

    let matcher = builder.build(pattern).map_err(|e| format!("Invalid regex pattern: {}", e))?;

    let search_path = Path::new(path);

    // For single-file searches, skip the walker
    if search_path.is_file() {
        let output = Arc::new(Mutex::new(Vec::<u8>::new()));
        search_single_file(search_path, &matcher, &output)?;
        let buf = output.lock().unwrap_or_else(|e| e.into_inner());
        return Ok(String::from_utf8_lossy(&buf).to_string());
    }

    // Build the directory walker
    let mut walker_builder = WalkBuilder::new(search_path);
    walker_builder
        .hidden(true) // skip hidden files
        .git_ignore(true) // respect .gitignore
        .git_global(true)
        .git_exclude(true)
        .ignore(true) // respect .ignore files
        .max_depth(None)
        .follow_links(false);

    // Apply glob filter
    if let Some(g) = glob {
        // Use an override to filter by glob
        let mut overrides = ignore::overrides::OverrideBuilder::new(search_path);
        overrides.add(g).map_err(|e| format!("Invalid glob pattern '{}': {}", g, e))?;
        let overrides = overrides.build().map_err(|e| format!("Failed to build glob: {}", e))?;
        walker_builder.overrides(overrides);
    }

    let output = Arc::new(Mutex::new(Vec::<u8>::with_capacity(64 * 1024)));
    let match_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // Walk and search
    for entry in walker_builder.build() {
        // Check cancellation periodically
        if cancel.is_cancelled() {
            return Err("Search cancelled".to_string());
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Skip directories
        if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            continue;
        }

        let file_path = entry.path();

        // Stream file path being searched
        let count = match_count.load(std::sync::atomic::Ordering::Relaxed);
        progress(&format!("{} ({} matches)", file_path.display(), count));

        // Search this file
        let mut searcher = Searcher::new();
        let out = Arc::clone(&output);
        let count = Arc::clone(&match_count);
        let file_path_str = file_path.display().to_string();

        let _ = searcher.search_path(
            &matcher,
            file_path,
            UTF8(|line_num, line| {
                let c = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                // Safety valve: stop after 10000 matches to prevent OOM
                if c >= 10_000 {
                    return Ok(false);
                }
                let mut buf = out.lock().unwrap_or_else(|e| e.into_inner());
                let _ = write!(buf, "{}:{}:{}", file_path_str, line_num, line);
                if !line.ends_with('\n') {
                    let _ = writeln!(buf);
                }
                Ok(true)
            }),
        );

        if match_count.load(std::sync::atomic::Ordering::Relaxed) >= 10_000 {
            let mut buf = output.lock().unwrap_or_else(|e| e.into_inner());
            let _ = writeln!(buf, "\n[Truncated: more than 10000 matches]");
            break;
        }
    }

    let buf = output.lock().unwrap_or_else(|e| e.into_inner());
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// Search a single file (not a directory walk).
fn search_single_file(path: &Path, matcher: &RegexMatcher, output: &Arc<Mutex<Vec<u8>>>) -> Result<(), String> {
    let mut searcher = Searcher::new();
    let path_str = path.display().to_string();
    let out = Arc::clone(output);

    searcher
        .search_path(
            matcher,
            path,
            UTF8(|line_num, line| {
                let mut buf = out.lock().unwrap_or_else(|e| e.into_inner());
                let _ = write!(buf, "{}:{}:{}", path_str, line_num, line);
                if !line.ends_with('\n') {
                    let _ = writeln!(buf);
                }
                Ok(true)
            }),
        )
        .map_err(|e| format!("Failed to search {}: {}", path_str, e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_current_dir() {
        let cancel = CancellationToken::new();
        // Search for something that definitely exists in this project
        let result = search_files("GrepTool", ".", Some("*.rs"), None, &cancel, |_| {});
        assert!(result.is_ok());
        let output = result.expect("search should succeed");
        assert!(output.contains("GrepTool"), "should find GrepTool in grep.rs");
    }

    #[test]
    fn test_search_no_matches() {
        let cancel = CancellationToken::new();
        // Use a single file to avoid matching the test source itself
        let result = search_files("zzz_definitely_not_in_any_file_zzz", "Cargo.toml", None, None, &cancel, |_| {});
        assert!(result.is_ok());
        assert!(result.expect("search should succeed").is_empty());
    }

    #[test]
    fn test_search_invalid_regex() {
        let cancel = CancellationToken::new();
        let result = search_files("[invalid", ".", None, None, &cancel, |_| {});
        assert!(result.is_err());
        assert!(result.expect_err("invalid regex should error").contains("Invalid regex"));
    }

    #[test]
    fn test_search_single_file() {
        let cancel = CancellationToken::new();
        let result = search_files("GrepTool", "src/tools/grep.rs", None, None, &cancel, |_| {});
        assert!(result.is_ok());
        let output = result.expect("search should succeed");
        assert!(output.contains("GrepTool"));
    }

    #[test]
    fn test_smart_case() {
        let cancel = CancellationToken::new();
        // Lowercase pattern -> case insensitive: should match "GrepTool"
        let result = search_files("greptool", "src/tools/grep.rs", None, None, &cancel, |_| {});
        assert!(result.is_ok());
        let output = result.expect("search should succeed");
        assert!(output.contains("GrepTool"), "smart case: lowercase should match GrepTool");

        // Uppercase pattern -> case sensitive: search Cargo.toml where "GREPTOOL" definitely doesn't appear
        let result2 = search_files("CLANKERS_NONEXISTENT_UPPER", "Cargo.toml", None, None, &cancel, |_| {});
        assert!(result2.is_ok());
        assert!(result2.expect("search should succeed").is_empty(), "case-sensitive uppercase pattern should not match");

        // Explicit case-insensitive override: "clankers" in lowercase should match "clankers" in Cargo.toml
        let result3 = search_files("CLANKERS", "Cargo.toml", None, Some(false), &cancel, |_| {});
        assert!(result3.is_ok());
        assert!(!result3.expect("search should succeed").is_empty(), "explicit case-insensitive should match 'clankers'");
    }

    #[test]
    fn test_cancellation() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = search_files("anything", ".", None, None, &cancel, |_| {});
        assert!(result.is_err());
        assert!(result.expect_err("cancelled search should error").contains("cancelled"));
    }
}
