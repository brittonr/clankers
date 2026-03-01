//! Search-and-replace edit

use std::path::Path;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use tokio::fs;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct EditTool {
    definition: ToolDefinition,
}

impl EditTool {
    pub fn new() -> Self {
        let definition = ToolDefinition {
            name: "edit".to_string(),
            description: "Edit a file by replacing exact text. The old_text must match exactly (including whitespace). Use this for precise, surgical edits.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to edit (relative or absolute)"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Exact text to find and replace (must match exactly)"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "New text to replace the old text with"
                    }
                },
                "required": ["path", "old_text", "new_text"]
            }),
        };

        Self { definition }
    }


}

impl Default for EditTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EditTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        // Parse parameters
        let path_str = match params.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::error("Missing required parameter: path"),
        };

        let old_text = match params.get("old_text").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return ToolResult::error("Missing required parameter: old_text"),
        };

        let new_text = match params.get("new_text").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return ToolResult::error("Missing required parameter: new_text"),
        };

        let path = Path::new(path_str);

        // Check if file exists
        if !path.exists() {
            return ToolResult::error(format!("File not found: {}", path_str));
        }

        if !path.is_file() {
            return ToolResult::error(format!("Not a file: {}", path_str));
        }

        // Read the file
        let content = match fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read file: {}", e)),
        };

        // Find and replace
        if !content.contains(old_text) {
            return ToolResult::error(format!(
                "old_text not found in file. Make sure the text matches exactly (including whitespace).\n\nSearching for:\n{}\n",
                old_text
            ));
        }

        // Count occurrences
        let occurrences = content.matches(old_text).count();
        if occurrences > 1 {
            return ToolResult::error(format!(
                "old_text appears {} times in the file. Edit tool requires exact match with single occurrence.\n\nSearching for:\n{}\n",
                occurrences, old_text
            ));
        }

        // Perform replacement
        let new_content = content.replace(old_text, new_text);

        // Stream diff preview before applying
        let diff = super::diff::unified_diff(path_str, &content, &new_content);
        if !diff.is_empty() {
            ctx.emit_progress(&diff);
        }

        // Write back to file
        if let Err(e) = fs::write(path, &new_content).await {
            return ToolResult::error(format!("Failed to write file: {}", e));
        }

        let stat = super::diff::diff_stat(path_str, &content, &new_content);
        ToolResult::text(format!("Successfully edited {}\n{}", path_str, stat))
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;
    use tokio::runtime::Runtime;
    use tokio_util::sync::CancellationToken;

    use super::super::ToolResultContent;
    use super::*;

    fn get_text_content(result: &ToolResult) -> Option<String> {
        result.content.first().and_then(|c| {
            if let ToolResultContent::Text { text } = c {
                Some(text.clone())
            } else {
                None
            }
        })
    }

    #[test]
    fn test_edit_simple_replacement() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let dir = TempDir::new().unwrap();
            let file_path = dir.path().join("test.txt");
            tokio::fs::write(&file_path, "hello world\nfoo bar").await.unwrap();

            let tool = EditTool::new();
            let params = json!({
                "path": file_path.to_str().unwrap(),
                "old_text": "hello world",
                "new_text": "goodbye world"
            });

            let result = tool.execute(&ToolContext::new("1".to_string(), CancellationToken::new(), None), params).await;
            assert!(!result.is_error);
            let text = get_text_content(&result).unwrap();
            assert!(text.contains("Successfully"));

            let content = tokio::fs::read_to_string(&file_path).await.unwrap();
            assert_eq!(content, "goodbye world\nfoo bar");
        });
    }

    #[test]
    fn test_edit_missing_file() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let tool = EditTool::new();
            let params = json!({
                "path": "/nonexistent/file.txt",
                "old_text": "foo",
                "new_text": "bar"
            });

            let result = tool.execute(&ToolContext::new("1".to_string(), CancellationToken::new(), None), params).await;
            assert!(result.is_error);
            let text = get_text_content(&result).unwrap();
            assert!(text.contains("not found"));
        });
    }

    #[test]
    fn test_edit_text_not_found() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let dir = TempDir::new().unwrap();
            let file_path = dir.path().join("test.txt");
            tokio::fs::write(&file_path, "hello world").await.unwrap();

            let tool = EditTool::new();
            let params = json!({
                "path": file_path.to_str().unwrap(),
                "old_text": "goodbye",
                "new_text": "hello"
            });

            let result = tool.execute(&ToolContext::new("1".to_string(), CancellationToken::new(), None), params).await;
            assert!(result.is_error);
            let text = get_text_content(&result).unwrap();
            assert!(text.contains("not found"));
        });
    }

    #[test]
    fn test_edit_multiple_occurrences() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let dir = TempDir::new().unwrap();
            let file_path = dir.path().join("test.txt");
            tokio::fs::write(&file_path, "foo\nfoo\nfoo").await.unwrap();

            let tool = EditTool::new();
            let params = json!({
                "path": file_path.to_str().unwrap(),
                "old_text": "foo",
                "new_text": "bar"
            });

            let result = tool.execute(&ToolContext::new("1".to_string(), CancellationToken::new(), None), params).await;
            assert!(result.is_error);
            let text = get_text_content(&result).unwrap();
            assert!(text.contains("appears 3 times"));
        });
    }
}
