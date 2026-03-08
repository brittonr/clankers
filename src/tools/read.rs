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

/// File type classification for reading.
enum FileType {
    Image,
    Binary,
    Text,
}

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

        // Validate file exists and is readable
        if let Some(error) = Self::validate_file(path, path_str) {
            return error;
        }

        // Handle different file types
        match Self::detect_file_type(path) {
            FileType::Image => Self::read_image_file(ctx, path, path_str).await,
            FileType::Binary => ToolResult::error("Binary file, cannot read"),
            FileType::Text => Self::read_text_file(ctx, path, path_str, offset, limit).await,
        }
    }
}

impl ReadTool {
    /// Validate that the path exists and is a regular file.
    fn validate_file(path: &Path, path_str: &str) -> Option<ToolResult> {
        if !path.exists() {
            return Some(ToolResult::error(format!("File not found: {}", path_str)));
        }

        if !path.is_file() {
            return Some(ToolResult::error(format!("Not a file: {}", path_str)));
        }

        None
    }

    /// Detect whether the file is an image, binary, or text.
    fn detect_file_type(path: &Path) -> FileType {
        if Self::is_image_file(path) {
            FileType::Image
        } else if let Ok(true) = is_binary_file(path) {
            FileType::Binary
        } else {
            FileType::Text
        }
    }

    /// Read and return an image file as base64.
    async fn read_image_file(ctx: &ToolContext, path: &Path, path_str: &str) -> ToolResult {
        ctx.emit_progress(&format!("reading image: {}", path_str));
        match fs::read(path).await {
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
        }
    }

    /// Read a text file with offset/limit and line numbering.
    async fn read_text_file(
        ctx: &ToolContext,
        path: &Path,
        path_str: &str,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> ToolResult {
        // Read file content
        let content = match fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read file: {}", e)),
        };

        // Apply offset and limit
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let (start, end) = match Self::calculate_line_range(offset, limit, total_lines) {
            Ok(range) => range,
            Err(e) => return e,
        };

        // Emit progress
        Self::emit_read_progress(ctx, path_str, start, end, total_lines, limit.is_some());

        // Format output with line numbers
        let output = Self::format_lines_with_numbers(ctx, &lines, start, end);

        // Apply truncation and return
        Self::apply_truncation(output)
    }

    /// Calculate the start and end line indices based on offset and limit.
    fn calculate_line_range(
        offset: Option<usize>,
        limit: Option<usize>,
        total_lines: usize,
    ) -> Result<(usize, usize), ToolResult> {
        let start = offset.unwrap_or(1).saturating_sub(1); // Convert to 0-indexed

        if start >= total_lines {
            return Err(ToolResult::error(format!(
                "Offset {} exceeds file length ({} lines)",
                offset.unwrap_or(1),
                total_lines
            )));
        }

        let end = if let Some(lim) = limit {
            std::cmp::min(start + lim, total_lines)
        } else {
            total_lines
        };

        Ok((start, end))
    }

    /// Emit progress indicating what range is being read.
    fn emit_read_progress(
        ctx: &ToolContext,
        path_str: &str,
        start: usize,
        end: usize,
        total_lines: usize,
        has_limit: bool,
    ) {
        let range_desc = if has_limit {
            format!("lines {}-{}/{}", start + 1, end, total_lines)
        } else {
            format!("{} lines", total_lines)
        };
        ctx.emit_progress(&format!("{} ({})", path_str, range_desc));
    }

    /// Threshold for considering a file "large" (for progress streaming)
    const LARGE_FILE_LINE_THRESHOLD: usize = 500;
    /// Progress emit interval for large files (emit every N lines)
    const PROGRESS_INTERVAL_LINES: usize = 200;

    /// Format lines with line numbers, emitting progress for large files.
    fn format_lines_with_numbers(
        ctx: &ToolContext,
        lines: &[&str],
        start: usize,
        end: usize,
    ) -> String {
        let mut output = String::new();
        let line_count = end - start;
        // Stream every N lines for large files
        let stream_interval = if line_count > Self::LARGE_FILE_LINE_THRESHOLD { 
            Self::PROGRESS_INTERVAL_LINES 
        } else { 
            usize::MAX 
        };

        for (idx, line) in lines[start..end].iter().enumerate() {
            let line_no = start + idx + 1;
            output.push_str(&format!("{} | {}\n", line_no, line));

            if (idx + 1) % stream_interval == 0 {
                ctx.emit_progress(&format!("{} | ...", line_no));
            }
        }

        output
    }

    /// Apply truncation limits and add truncation notice if needed.
    fn apply_truncation(output: String) -> ToolResult {
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
