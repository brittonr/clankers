//! Tool result types
//!
//! Defines the result structure returned by tool executions.
//! Part of the message protocol between the agent and tools.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ToolResultContent>,
    pub is_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    /// If output was truncated, path to the full output file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_output_path: Option<String>,
}

/// Content block within a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolResultContent {
    Text { text: String },
    Image { media_type: String, data: String },
}

impl ToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolResultContent::Text { text: text.into() }],
            is_error: false,
            details: None,
            full_output_path: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolResultContent::Text { text: message.into() }],
            is_error: true,
            details: None,
            full_output_path: None,
        }
    }

    /// Add details metadata to this result
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}
