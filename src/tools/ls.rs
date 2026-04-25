//! Directory listing

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use tokio::fs;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct LsTool {
    definition: ToolDefinition,
}

impl LsTool {
    pub fn new() -> Self {
        let definition = ToolDefinition {
            name: "ls".to_string(),
            description: "List directory contents. Returns sorted entries, one per line. Directories have trailing '/'. Includes dotfiles.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path (default: current directory)"
                    }
                },
                "required": []
            }),
        };

        Self { definition }
    }
}

impl Default for LsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for LsTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        // Parse parameters
        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        ctx.emit_progress(&format!("listing: {}", path));

        // Read directory
        let mut entries = match fs::read_dir(path).await {
            Ok(e) => e,
            Err(e) => return ToolResult::error(format!("Failed to read directory: {}", e)),
        };

        // Collect all entries
        let mut items = Vec::new();
        while let Some(entry) = entries.next_entry().await.transpose() {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => return ToolResult::error(format!("Failed to read directory entry: {}", e)),
            };

            let name = entry.file_name().to_string_lossy().to_string();

            // Check if it's a directory
            let is_dir = match entry.file_type().await {
                Ok(ft) => ft.is_dir(),
                Err(_) => false,
            };

            let formatted = if is_dir { format!("{}/", name) } else { name };

            items.push(formatted);
        }

        if items.is_empty() {
            return ToolResult::text("(empty directory)");
        }

        // Sort entries
        items.sort_unstable();

        // Stream count
        ctx.emit_progress(&format!("{}: {} entries", path, items.len()));

        // Format output
        let output = items.join("\n");

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
}
