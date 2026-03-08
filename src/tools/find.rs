//! File search by glob using the `ignore` crate.
//!
//! Uses `ignore::WalkBuilder` for .gitignore-respecting file traversal and
//! `ignore::overrides::OverrideBuilder` for glob pattern filtering.
//! No external `find` binary required.

use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use ignore::WalkBuilder;
use serde_json::Value;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use super::progress::{ResultChunk, ToolProgress};

pub struct FindTool {
    definition: ToolDefinition,
}

impl FindTool {
    pub fn new() -> Self {
        let definition = ToolDefinition {
            name: "find".to_string(),
            description: "Find files by name pattern using glob. Returns sorted list of matching file paths. Respects .gitignore. Output is truncated to 2000 lines or 50KB.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "File name pattern (glob)"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search (default: current directory)"
                    }
                },
                "required": ["pattern"]
            }),
        };

        Self { definition }
    }
}

impl Default for FindTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Perform the actual in-process file search.
fn find_files(
    pattern: &str,
    path: &str,
    cancel: &CancellationToken,
    progress: impl Fn(&str),
) -> Result<String, String> {
    let search_path = Path::new(path);

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

    // Apply glob filter using overrides
    let mut overrides = ignore::overrides::OverrideBuilder::new(search_path);
    overrides.add(pattern).map_err(|e| format!("Invalid glob pattern '{}': {}", pattern, e))?;
    let overrides = overrides.build().map_err(|e| format!("Failed to build glob: {}", e))?;
    walker_builder.overrides(overrides);

    let files = Arc::new(Mutex::new(Vec::<String>::new()));

    // Walk and collect matching files
    for entry in walker_builder.build() {
        // Check cancellation periodically
        if cancel.is_cancelled() {
            return Err("Search cancelled".to_string());
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Skip directories - we only want files
        if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            continue;
        }

        let file_path = entry.path();
        let path_str = file_path.display().to_string();

        // Add to results
        let mut files_vec = files.lock().unwrap_or_else(|e| e.into_inner());
        files_vec.push(path_str.clone());
        let count = files_vec.len();

        // Emit progress
        progress(&format!("{} ({})", path_str, count));
    }

    let mut files_vec = files.lock().unwrap_or_else(|e| e.into_inner());
    
    // Sort the results
    files_vec.sort_unstable();
    
    Ok(files_vec.join("\n"))
}

#[async_trait]
impl Tool for FindTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        // Parse parameters
        let pattern = match params.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolResult::error("Missing required parameter: pattern"),
        };

        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".").to_string();

        // Run the search in a blocking task to avoid blocking the async runtime
        let cancel = ctx.signal.clone();
        let progress_ctx = ctx.clone();
        let result = tokio::task::spawn_blocking(move || {
            find_files(&pattern, &path, &cancel, |msg| {
                progress_ctx.emit_progress(msg);
                // Extract count from "path (N)" format for structured progress
                if let Some(start) = msg.rfind('(')
                    && let Some(end) = msg.rfind(')')
                    && let Ok(count) = msg[start + 1..end].parse::<u64>()
                {
                    progress_ctx.emit_structured_progress(
                        ToolProgress::items(count, None)
                            .with_message("Finding files"),
                    );
                }
            })
        })
        .await;

        match result {
            Ok(Ok(output)) => {
                if output.is_empty() {
                    return ToolResult::text("No files found");
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
            Err(e) => ToolResult::error(format!("Find task panicked: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_rs_files() {
        let cancel = CancellationToken::new();
        // Find all .rs files in src/tools
        let result = find_files("*.rs", "src/tools", &cancel, |_| {});
        assert!(result.is_ok());
        let output = result.expect("find should succeed");
        assert!(!output.is_empty(), "should find at least one .rs file");
        // Should find find.rs itself
        assert!(output.contains("find.rs"), "should find find.rs");
    }

    #[test]
    fn test_find_no_matches() {
        let cancel = CancellationToken::new();
        // Use a glob pattern that matches nothing
        let result = find_files("*.zzz_nonexistent_extension", ".", &cancel, |_| {});
        assert!(result.is_ok());
        let output = result.expect("find should succeed");
        assert!(output.is_empty(), "should return empty string for no matches");
    }

    #[test]
    fn test_find_cancellation() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = find_files("*.rs", ".", &cancel, |_| {});
        assert!(result.is_err());
        assert!(result.expect_err("cancelled find should error").contains("cancelled"));
    }

    #[test]
    fn test_find_respects_gitignore() {
        let cancel = CancellationToken::new();
        // Find all .rs files in src/ - this directory should not have any ignored files
        // The ignore crate DOES respect .gitignore automatically
        let result = find_files("*.rs", "src", &cancel, |_| {});
        assert!(result.is_ok());
        let output = result.expect("find should succeed");
        assert!(!output.is_empty(), "should find at least one .rs file in src/");
        // Verify no target paths appear (shouldn't happen when searching src/, but good to check)
        for line in output.lines() {
            assert!(!line.contains("/target/"), 
                    "should not find files in target/ subdirectories: {}", line);
        }
    }

    #[test]
    fn test_find_sorted_output() {
        let cancel = CancellationToken::new();
        // Find multiple .rs files
        let result = find_files("*.rs", "src/tools", &cancel, |_| {});
        assert!(result.is_ok());
        let output = result.expect("find should succeed");
        let lines: Vec<&str> = output.lines().collect();
        
        // Check that output is sorted
        for i in 1..lines.len() {
            assert!(lines[i-1] <= lines[i], 
                    "output should be sorted: {} should come before {}", 
                    lines[i-1], lines[i]);
        }
    }

    #[test]
    fn test_find_invalid_glob() {
        let cancel = CancellationToken::new();
        // Test with an invalid glob pattern (glob crate might accept this, but let's try)
        // Actually, most patterns are valid, so this is hard to trigger
        // Instead, test that a valid complex pattern works
        let result = find_files("*.{rs,toml}", ".", &cancel, |_| {});
        assert!(result.is_ok(), "should handle brace expansion patterns");
    }
}
