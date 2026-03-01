//! Read file contents

use std::path::Path;

use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose;
use serde_json::Value;
use serde_json::json;
use tokio::fs;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use super::ToolResultContent;
use crate::util::fs::is_binary_file;

pub struct ReadTool {
    definition: ToolDefinition,
}

impl ReadTool {
    pub fn new() -> Self {
        let definition = ToolDefinition {
            name: "read".to_string(),
            description: "Read file contents with line numbers. Supports text files and images (jpg, png, gif, webp). Images are returned as base64. For text files, output is truncated to 2000 lines or 50KB (whichever is hit first). Use offset/limit for large files.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read (relative or absolute)"
                    },
                    "offset": {
                        "type": "number",
                        "description": "Line number to start reading from (1-indexed)"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of lines to read"
                    }
                },
                "required": ["path"]
            }),
        };

        Self { definition }
    }

    fn is_image_file(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp")
        } else {
            false
        }
    }

    fn get_image_media_type(path: &Path) -> String {
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            match ext.as_str() {
                "jpg" | "jpeg" => "image/jpeg".to_string(),
                "png" => "image/png".to_string(),
                "gif" => "image/gif".to_string(),
                "webp" => "image/webp".to_string(),
                _ => "application/octet-stream".to_string(),
            }
        } else {
            "application/octet-stream".to_string()
        }
    }
}

impl Default for ReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        // Parse parameters
        let path_str = match params.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::error("Missing required parameter: path"),
        };

        let offset = params.get("offset").and_then(|v| v.as_u64()).map(|v| v as usize);

        let limit = params.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);

        let path = Path::new(path_str);

        // Check if file exists
        if !path.exists() {
            return ToolResult::error(format!("File not found: {}", path_str));
        }

        if !path.is_file() {
            return ToolResult::error(format!("Not a file: {}", path_str));
        }

        // Handle image files
        if Self::is_image_file(path) {
            ctx.emit_progress(&format!("reading image: {}", path_str));
            return match fs::read(path).await {
                Ok(bytes) => {
                    let base64_data = general_purpose::STANDARD.encode(&bytes);
                    let media_type = Self::get_image_media_type(path);
                    ToolResult {
                        content: vec![ToolResultContent::Image {
                            media_type,
                            data: base64_data,
                        }],
                        is_error: false,
                        details: None,
                        full_output_path: None,
                    }
                }
                Err(e) => ToolResult::error(format!("Failed to read image: {}", e)),
            };
        }

        // Check if binary
        if let Ok(true) = is_binary_file(path) {
            return ToolResult::error("Binary file, cannot read");
        }

        // Read text file
        let content = match fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read file: {}", e)),
        };

        // Split into lines
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Apply offset and limit
        let start = offset.unwrap_or(1).saturating_sub(1); // Convert to 0-indexed
        let end = if let Some(lim) = limit {
            std::cmp::min(start + lim, total_lines)
        } else {
            total_lines
        };

        if start >= total_lines {
            return ToolResult::error(format!(
                "Offset {} exceeds file length ({} lines)",
                offset.unwrap_or(1),
                total_lines
            ));
        }

        // Stream progress: file name and line range
        let range_desc = if limit.is_some() {
            format!("lines {}-{}/{}", start + 1, end, total_lines)
        } else {
            format!("{} lines", total_lines)
        };
        ctx.emit_progress(&format!("{} ({})", path_str, range_desc));

        // Format with line numbers, streaming chunks for large files
        let mut output = String::new();
        let line_count = end - start;
        // Stream every 200 lines for large files
        let stream_interval = if line_count > 500 { 200 } else { usize::MAX };

        for (idx, line) in lines[start..end].iter().enumerate() {
            let line_no = start + idx + 1;
            output.push_str(&format!("{} | {}\n", line_no, line));

            if (idx + 1) % stream_interval == 0 {
                ctx.emit_progress(&format!("{} | ...", line_no));
            }
        }

        // Apply truncation (2000 lines or 50KB)
        const MAX_LINES: usize = 2000;
        const MAX_BYTES: usize = 50 * 1024;

        let (truncated_output, full_output_path) =
            crate::tools::truncation::truncate_head(&output, MAX_LINES, MAX_BYTES);

        let mut result = ToolResult::text(truncated_output);

        if let Some(path) = full_output_path {
            result.full_output_path = Some(path.display().to_string());
            result.content[0] = ToolResultContent::Text {
                text: format!(
                    "{}\n\n[Output truncated at {} lines / {} bytes. Use offset/limit to read more.]",
                    match &result.content[0] {
                        ToolResultContent::Text { text } => text.as_str(),
                        _ => "",
                    },
                    MAX_LINES,
                    MAX_BYTES
                ),
            };
        }

        result
    }
}
