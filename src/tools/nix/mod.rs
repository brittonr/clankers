//! Nix build/develop/run tool with structured output streaming
//!
//! Parses nix's `--log-format internal-json` to provide clean, meaningful
//! progress updates instead of raw terminal noise. Supports all common nix
//! subcommands: build, develop, run, shell, flake check/show/update.

mod parser;
mod build;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

use build::{supports_structured_logging, spawn_nix_command, stream_nix_output, format_and_truncate_result};

// ── Tool implementation ─────────────────────────────────────────────────────

pub struct NixTool {
    definition: ToolDefinition,
}

impl NixTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "nix".to_string(),
                description: "Run nix commands with streaming build output. Supports build, develop, run, shell, flake, eval, and other nix subcommands. Parses nix's internal-json structured logging for clean progress display (builds, downloads, fetches, phases). Use this instead of bash for nix commands.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "subcommand": {
                            "type": "string",
                            "description": "Nix subcommand (build, develop, run, shell, flake, eval, store, etc.)"
                        },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Arguments to pass after the subcommand (e.g. [\".#myPackage\", \"--no-link\"])"
                        },
                        "timeout": {
                            "type": "number",
                            "description": "Timeout in seconds (default: 600 for builds, 0 = no timeout)"
                        }
                    },
                    "required": ["subcommand"]
                }),
            },
        }
    }
}

impl Default for NixTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for NixTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let subcommand = match params.get("subcommand").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return ToolResult::error("Missing required parameter: subcommand"),
        };

        let args: Vec<String> = params
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        let timeout_secs = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(0);

        // Decide whether to use structured logging
        let use_structured = supports_structured_logging(&subcommand);

        // Spawn the nix command
        let mut child = match spawn_nix_command(&subcommand, &args, use_structured) {
            Ok(c) => c,
            Err(e) => return ToolResult::error(e),
        };

        // Stream output and collect results
        let (exit_code, stdout_lines, build_log_lines, messages, errors) = 
            match stream_nix_output(ctx, &mut child, use_structured, timeout_secs, &subcommand).await {
                Ok(result) => result,
                Err(e) => return e,
            };

        // Format and truncate the result
        format_and_truncate_result(&subcommand, exit_code, &stdout_lines, &build_log_lines, &messages, &errors)
    }
}
