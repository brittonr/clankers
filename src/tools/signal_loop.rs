//! Signal loop success — the LLM calls this to break out of a loop.
//!
//! This tool does not execute anything. It simply returns a confirmation
//! message. The event loop detects that this tool was called (via
//! `AgentEvent::ToolCall`) and breaks the active loop.

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct SignalLoopTool {
    definition: ToolDefinition,
}

impl SignalLoopTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "signal_loop_success".to_string(),
                description: concat!(
                    "Stop the active loop when the breakout condition is satisfied. ",
                    "Only call this tool when explicitly instructed to do so by the ",
                    "user, tool, or system prompt.",
                )
                .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        }
    }
}

#[async_trait]
impl Tool for SignalLoopTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, _params: Value) -> ToolResult {
        ToolResult::text("Loop break signaled.")
    }
}
