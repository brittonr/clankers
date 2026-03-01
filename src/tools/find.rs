//! File search by glob

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::process::Command;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct FindTool {
    definition: ToolDefinition,
}

impl FindTool {
    pub fn new() -> Self {
        let definition = ToolDefinition {
            name: "find".to_string(),
            description: "Find files by name pattern using Unix find. Returns sorted list of matching file paths. Output is truncated to 2000 lines or 50KB.".to_string(),
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

#[async_trait]
impl Tool for FindTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        // Parse parameters
        let pattern = match params.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::error("Missing required parameter: pattern"),
        };

        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        // Build find command
        let mut cmd = Command::new("find");
        cmd.arg(path)
            .arg("-name")
            .arg(pattern)
            .arg("-type")
            .arg("f")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Spawn process
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to spawn find: {}", e)),
        };

        // Stream stdout line-by-line for live progress
        let stdout_handle = match child.stdout.take() {
            Some(s) => s,
            None => return ToolResult::error("Failed to capture stdout from find process"),
        };
        let mut stderr_handle = match child.stderr.take() {
            Some(s) => s,
            None => return ToolResult::error("Failed to capture stderr from find process"),
        };

        let mut reader = BufReader::new(stdout_handle).lines();
        let mut collected = Vec::new();

        loop {
            tokio::select! {
                _ = ctx.signal.cancelled() => {
                    let _ = child.start_kill();
                    return ToolResult::error("Search cancelled");
                }
                line = reader.next_line() => {
                    match line {
                        Ok(Some(line)) => {
                            ctx.emit_progress(&format!("{} ({})", line, collected.len() + 1));
                            collected.push(line);
                        }
                        Ok(None) => break,
                        Err(e) => return ToolResult::error(format!("Read error: {}", e)),
                    }
                }
            }
        }

        let _ = child.wait().await;

        // Collect stderr
        let mut stderr_buf = Vec::new();
        let _ = stderr_handle.read_to_end(&mut stderr_buf).await;

        let mut stdout = collected.join("\n");

        // Check for errors in stderr
        if !stderr_buf.is_empty() {
            let stderr = String::from_utf8_lossy(&stderr_buf);
            if !stderr.lines().all(|line| line.contains("Permission denied")) {
                if !stdout.is_empty() {
                    stdout.push_str("\n\nSTDERR:\n");
                }
                stdout.push_str(&stderr);
            }
        }

        if stdout.trim().is_empty() {
            return ToolResult::text("No files found");
        }

        // Sort the output
        collected.sort_unstable();
        let sorted_output = collected.join("\n");

        // Apply truncation
        const MAX_LINES: usize = 2000;
        const MAX_BYTES: usize = 50 * 1024;

        let (truncated_output, full_output_path) =
            crate::tools::truncation::truncate_tail(&sorted_output, MAX_LINES, MAX_BYTES);

        let mut result = ToolResult::text(truncated_output);
        if let Some(path) = full_output_path {
            result.full_output_path = Some(path.display().to_string());
        }

        result
    }
}
