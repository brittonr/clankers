//! Working-directory checkpoint and rollback tool.

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct CheckpointTool {
    definition: ToolDefinition,
}

impl CheckpointTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "checkpoint".to_string(),
                description: "Create, list, or rollback local git-backed working-directory checkpoints.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["create", "list", "rollback"],
                            "description": "Checkpoint action to perform"
                        },
                        "label": {
                            "type": "string",
                            "description": "Optional label when creating a checkpoint"
                        },
                        "checkpoint_id": {
                            "type": "string",
                            "description": "Checkpoint id for rollback"
                        },
                        "confirm": {
                            "type": "boolean",
                            "description": "Required true for rollback"
                        }
                    },
                    "required": ["action"]
                }),
            },
        }
    }
}

impl Default for CheckpointTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CheckpointTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let action = match params.get("action").and_then(|v| v.as_str()) {
            Some(action) => action,
            None => return ToolResult::error("Missing required parameter: action"),
        };
        let cwd = match std::env::current_dir() {
            Ok(cwd) => cwd,
            Err(error) => return ToolResult::error(format!("Failed to resolve current directory: {error}")),
        };

        let outcome = match action {
            "create" => {
                let label = params.get("label").and_then(|v| v.as_str()).map(ToString::to_string);
                crate::checkpoints::create_checkpoint(&cwd, label)
            }
            "list" => crate::checkpoints::list_checkpoints(&cwd),
            "rollback" => {
                let checkpoint_id = match params.get("checkpoint_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return ToolResult::error("Missing required parameter for rollback: checkpoint_id"),
                };
                let confirm = params.get("confirm").and_then(|v| v.as_bool()).unwrap_or(false);
                crate::checkpoints::rollback_checkpoint(&cwd, checkpoint_id, confirm)
            }
            other => return ToolResult::error(format!("Unknown checkpoint action: {other}")),
        };

        match outcome {
            Ok(outcome) => {
                let text = summarize(&outcome);
                ToolResult::text(text).with_details(outcome.details.to_details())
            }
            Err(error) => ToolResult::error(error.to_string()).with_details(
                crate::checkpoints::CheckpointMetadata::error(
                    action,
                    cwd.display().to_string(),
                    "checkpoint_error",
                    &error.to_string(),
                )
                .to_details(),
            ),
        }
    }
}

fn summarize(outcome: &crate::checkpoints::CheckpointOutcome) -> String {
    match outcome.action.as_str() {
        "create" => outcome
            .record
            .as_ref()
            .map(|record| format!("Created checkpoint {} ({} file(s))", record.id, record.changed_file_count))
            .unwrap_or_else(|| "Created checkpoint".to_string()),
        "rollback" => outcome
            .record
            .as_ref()
            .map(|record| format!("Rolled back to checkpoint {} ({} file(s))", record.id, record.changed_file_count))
            .unwrap_or_else(|| "Rolled back checkpoint".to_string()),
        "list" => format!("{} checkpoint(s)", outcome.records.len()),
        _ => outcome.status.clone(),
    }
}
