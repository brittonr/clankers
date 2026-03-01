//! Write file contents

use std::path::Path;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use tokio::fs;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct WriteTool {
    definition: ToolDefinition,
}

impl WriteTool {
    pub fn new() -> Self {
        let definition = ToolDefinition {
            name: "write".to_string(),
            description: "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write (relative or absolute)"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
        };

        Self { definition }
    }
}

impl Default for WriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        // Parse parameters
        let path_str = match params.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::error("Missing required parameter: path"),
        };

        let content = match params.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::error("Missing required parameter: content"),
        };

        let path = Path::new(path_str);

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent()
            && !parent.exists()
            && let Err(e) = fs::create_dir_all(parent).await
        {
            return ToolResult::error(format!("Failed to create parent directories: {}", e));
        }

        // Stream diff preview: compare against existing file (if any)
        let old_content = if path.is_file() {
            fs::read_to_string(path).await.unwrap_or_default()
        } else {
            String::new()
        };

        let diff = super::diff::unified_diff(path_str, &old_content, content);
        if !diff.is_empty() {
            ctx.emit_progress(&diff);
        }

        // Write the file
        match fs::write(path, content).await {
            Ok(_) => {
                let byte_count = content.len();
                if old_content.is_empty() {
                    ToolResult::text(format!("Created {} ({} bytes)", path_str, byte_count))
                } else {
                    let stat = super::diff::diff_stat(path_str, &old_content, content);
                    ToolResult::text(format!("Wrote {} ({} bytes)\n{}", path_str, byte_count, stat))
                }
            }
            Err(e) => ToolResult::error(format!("Failed to write file: {}", e)),
        }
    }
}
