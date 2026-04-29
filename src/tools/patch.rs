//! Hermes-compatible targeted patch tool.
//!
//! Supports exact/fuzzy-ish string replacement and a small V4A-style multi-file
//! patch format for update-file hunks.

use std::path::Path;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use tokio::fs;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct PatchTool {
    definition: ToolDefinition,
}

impl PatchTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "patch".to_string(),
                description: concat!(
                    "Targeted file patching. In replace mode, replace a unique old_string with ",
                    "new_string, or all occurrences when replace_all=true. Uses exact matching ",
                    "first, then CRLF/LF and trailing-whitespace-normalized matching. In patch ",
                    "mode, apply a V4A-style multi-file update patch. Emits a unified diff before writing."
                )
                .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "mode": {
                            "type": "string",
                            "enum": ["replace", "patch"],
                            "description": "Edit mode: replace (default) or patch"
                        },
                        "path": {
                            "type": "string",
                            "description": "File path for replace mode"
                        },
                        "old_string": {
                            "type": "string",
                            "description": "Text to find in replace mode"
                        },
                        "new_string": {
                            "type": "string",
                            "description": "Replacement text in replace mode"
                        },
                        "replace_all": {
                            "type": "boolean",
                            "description": "Replace all occurrences instead of requiring a unique match"
                        },
                        "patch": {
                            "type": "string",
                            "description": "V4A-style multi-file patch content for patch mode"
                        }
                    },
                    "required": []
                }),
            },
        }
    }

    async fn handle_replace(ctx: &ToolContext, params: &Value) -> ToolResult {
        let path_str = match params.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.is_empty() => path,
            _ => return ToolResult::error("Missing required parameter: path"),
        };
        let old_string = match params.get("old_string").and_then(|v| v.as_str()) {
            Some(text) => text,
            None => return ToolResult::error("Missing required parameter: old_string"),
        };
        let new_string = match params.get("new_string").and_then(|v| v.as_str()) {
            Some(text) => text,
            None => return ToolResult::error("Missing required parameter: new_string"),
        };
        let replace_all = params.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);
        apply_replacement(ctx, path_str, old_string, new_string, replace_all).await
    }

    async fn handle_patch(ctx: &ToolContext, params: &Value) -> ToolResult {
        let patch_text = match params.get("patch").and_then(|v| v.as_str()) {
            Some(text) if !text.trim().is_empty() => text,
            _ => return ToolResult::error("Missing required parameter: patch"),
        };
        let edits = match parse_v4a_patch(patch_text) {
            Ok(edits) => edits,
            Err(err) => return ToolResult::error(err),
        };
        if edits.is_empty() {
            return ToolResult::error("Patch contained no update hunks.");
        }

        let mut summaries = Vec::new();
        for edit in edits {
            let result = apply_replacement(ctx, &edit.path, &edit.old_text, &edit.new_text, false).await;
            if result.is_error {
                return result;
            }
            summaries.push(format!("Patched {}", edit.path));
        }
        ToolResult::text(summaries.join("\n"))
    }
}

impl Default for PatchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for PatchTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        match params.get("mode").and_then(|v| v.as_str()).unwrap_or("replace") {
            "replace" => Self::handle_replace(ctx, &params).await,
            "patch" => Self::handle_patch(ctx, &params).await,
            other => ToolResult::error(format!("Unknown patch mode: {other}")),
        }
    }
}

async fn apply_replacement(
    ctx: &ToolContext,
    path_str: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> ToolResult {
    if old_string.is_empty() {
        return ToolResult::error("old_string must not be empty.");
    }
    let path = Path::new(path_str);
    if !path.exists() {
        return ToolResult::error(format!("File not found: {path_str}"));
    }
    if !path.is_file() {
        return ToolResult::error(format!("Not a file: {path_str}"));
    }
    let content = match fs::read_to_string(path).await {
        Ok(content) => content,
        Err(err) => return ToolResult::error(format!("Failed to read file: {err}")),
    };

    let new_content = match replace_content(&content, old_string, new_string, replace_all) {
        Ok(content) => content,
        Err(err) => return ToolResult::error(err),
    };

    if new_content == content {
        return ToolResult::text(format!("No changes for {path_str}."));
    }

    let diff = super::diff::unified_diff(path_str, &content, &new_content);
    if !diff.is_empty() {
        ctx.emit_progress(&diff);
    }

    if let Err(err) = fs::write(path, &new_content).await {
        return ToolResult::error(format!("Failed to write file: {err}"));
    }
    let stat = super::diff::diff_stat(path_str, &content, &new_content);
    ToolResult::text(format!("Patched {path_str}\n{stat}"))
}

fn replace_content(content: &str, old_string: &str, new_string: &str, replace_all: bool) -> Result<String, String> {
    if replace_all {
        if !content.contains(old_string) {
            return fuzzy_replace(content, old_string, new_string, true);
        }
        return Ok(content.replace(old_string, new_string));
    }

    let count = content.matches(old_string).count();
    match count {
        1 => Ok(content.replacen(old_string, new_string, 1)),
        0 => fuzzy_replace(content, old_string, new_string, false),
        n => Err(format!(
            "old_string appears {n} times in the file. Use replace_all=true or provide more surrounding context."
        )),
    }
}

fn fuzzy_replace(content: &str, old_string: &str, new_string: &str, replace_all: bool) -> Result<String, String> {
    let normalized_old = normalize_newlines(old_string);
    let normalized_content = normalize_newlines(content);
    if normalized_old != old_string || normalized_content != content {
        return replace_content(&normalized_content, &normalized_old, new_string, replace_all);
    }

    let trimmed_old = trim_trailing_ws_by_line(old_string);
    let ranges = find_trailing_ws_insensitive_ranges(content, &trimmed_old);
    if ranges.is_empty() {
        return Err(format!(
            "old_string not found. Tried exact, CRLF/LF-normalized, and trailing-whitespace-normalized matching.\n\nSearching for:\n{old_string}"
        ));
    }
    if !replace_all && ranges.len() > 1 {
        return Err(format!(
            "old_string matched {} trailing-whitespace-normalized ranges. Provide more context or use replace_all=true.",
            ranges.len()
        ));
    }
    Ok(replace_ranges(content, &ranges, new_string, replace_all))
}

fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n")
}

fn trim_trailing_ws_by_line(value: &str) -> String {
    value.lines().map(str::trim_end).collect::<Vec<_>>().join("\n")
}

fn find_trailing_ws_insensitive_ranges(content: &str, old_trimmed: &str) -> Vec<(usize, usize)> {
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let old_lines: Vec<&str> = old_trimmed.split('\n').collect();
    if old_lines.is_empty() {
        return Vec::new();
    }
    let mut offsets = Vec::with_capacity(lines.len());
    let mut cursor = 0usize;
    for line in &lines {
        offsets.push(cursor);
        cursor += line.len();
    }
    let mut ranges = Vec::new();
    for start in 0..lines.len() {
        if start + old_lines.len() > lines.len() {
            break;
        }
        let window = &lines[start..start + old_lines.len()];
        let normalized = window
            .iter()
            .map(|line| line.trim_end_matches('\n').trim_end_matches('\r').trim_end())
            .collect::<Vec<_>>()
            .join("\n");
        if normalized == old_trimmed {
            let byte_start = offsets[start];
            let byte_end = if start + old_lines.len() < offsets.len() {
                offsets[start + old_lines.len()]
            } else {
                content.len()
            };
            ranges.push((byte_start, byte_end));
        }
    }
    ranges
}

fn replace_ranges(content: &str, ranges: &[(usize, usize)], new_string: &str, replace_all: bool) -> String {
    let selected: Vec<(usize, usize)> = if replace_all { ranges.to_vec() } else { vec![ranges[0]] };
    let mut out = String::new();
    let mut cursor = 0usize;
    for (start, end) in selected {
        out.push_str(&content[cursor..start]);
        out.push_str(new_string);
        cursor = end;
    }
    out.push_str(&content[cursor..]);
    out
}

#[derive(Debug)]
struct PatchEdit {
    path: String,
    old_text: String,
    new_text: String,
}

fn parse_v4a_patch(patch_text: &str) -> Result<Vec<PatchEdit>, String> {
    let mut edits = Vec::new();
    let mut current_path: Option<String> = None;
    let mut removed: Vec<String> = Vec::new();
    let mut added: Vec<String> = Vec::new();

    for raw_line in patch_text.lines() {
        if let Some(path) = raw_line.strip_prefix("*** Update File: ") {
            flush_patch_edit(&mut edits, &mut current_path, &mut removed, &mut added)?;
            current_path = Some(path.trim().to_string());
            continue;
        }
        if raw_line.starts_with("*** ") || raw_line.starts_with("@@") {
            continue;
        }
        if current_path.is_none() {
            continue;
        }
        if let Some(line) = raw_line.strip_prefix('-') {
            removed.push(line.to_string());
        } else if let Some(line) = raw_line.strip_prefix('+') {
            added.push(line.to_string());
        }
    }
    flush_patch_edit(&mut edits, &mut current_path, &mut removed, &mut added)?;
    Ok(edits)
}

fn flush_patch_edit(
    edits: &mut Vec<PatchEdit>,
    current_path: &mut Option<String>,
    removed: &mut Vec<String>,
    added: &mut Vec<String>,
) -> Result<(), String> {
    let Some(path) = current_path.take() else {
        return Ok(());
    };
    if removed.is_empty() && added.is_empty() {
        return Ok(());
    }
    if removed.is_empty() {
        return Err(format!(
            "Patch for {path} has additions but no removed anchor lines; create-file hunks are not supported yet."
        ));
    }
    edits.push(PatchEdit {
        path,
        old_text: format_patch_block(removed),
        new_text: format_patch_block(added),
    });
    removed.clear();
    added.clear();
    Ok(())
}

fn format_patch_block(lines: &[String]) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        let mut text = lines.join("\n");
        text.push('\n');
        text
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    use super::super::ToolResultContent;
    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::new("patch-test".to_string(), CancellationToken::new(), None)
    }

    fn text(result: &ToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|content| match content {
                ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[tokio::test]
    async fn replace_unique_string() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "hello world\n").await.unwrap();
        let result = PatchTool::new()
            .execute(&ctx(), json!({"mode": "replace", "path": path, "old_string": "hello", "new_string": "goodbye"}))
            .await;
        assert!(!result.is_error, "{result:?}");
        assert_eq!(fs::read_to_string(&path).await.unwrap(), "goodbye world\n");
    }

    #[tokio::test]
    async fn duplicate_errors_without_replace_all() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "x\nx\n").await.unwrap();
        let result =
            PatchTool::new().execute(&ctx(), json!({"path": path, "old_string": "x", "new_string": "y"})).await;
        assert!(result.is_error);
        assert!(text(&result).contains("appears 2 times"));
    }

    #[tokio::test]
    async fn replace_all_replaces_duplicates() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "x\nx\n").await.unwrap();
        let result = PatchTool::new()
            .execute(&ctx(), json!({"path": path, "old_string": "x", "new_string": "y", "replace_all": true}))
            .await;
        assert!(!result.is_error, "{result:?}");
        assert_eq!(fs::read_to_string(&path).await.unwrap(), "y\ny\n");
    }

    #[tokio::test]
    async fn trailing_whitespace_match_works() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "alpha   \nbeta\n").await.unwrap();
        let result = PatchTool::new()
            .execute(&ctx(), json!({"path": path, "old_string": "alpha\nbeta", "new_string": "gamma\n"}))
            .await;
        assert!(!result.is_error, "{result:?}");
        assert_eq!(fs::read_to_string(&path).await.unwrap(), "gamma\n");
    }

    #[tokio::test]
    async fn applies_v4a_update_patch() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "one\ntwo\nthree\n").await.unwrap();
        let patch = format!("*** Begin Patch\n*** Update File: {}\n@@\n-two\n+TWO\n*** End Patch", path.display());
        let result = PatchTool::new().execute(&ctx(), json!({"mode": "patch", "patch": patch})).await;
        assert!(!result.is_error, "{result:?}");
        assert_eq!(fs::read_to_string(&path).await.unwrap(), "one\nTWO\nthree\n");
    }
}
