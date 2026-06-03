//! Stable provider/router-neutral content block contracts.
//!
//! These DTOs describe message content that generic SDK crates may exchange
//! without inheriting Clankers transcript IDs, timestamps, session storage, or
//! desktop history records.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

/// A content block within a model, tool, or host message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Content {
    Text {
        text: String,
    },
    Image {
        source: ImageSource,
    },
    Thinking {
        thinking: String,
        /// Opaque signature returned by Anthropic; must be echoed back verbatim.
        #[serde(default = "empty_thinking_signature", skip_serializing_if = "String::is_empty")]
        signature: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: Vec<Content>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

fn empty_thinking_signature() -> String {
    String::new()
}

/// Image source (base64-encoded or URL).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImageSource {
    #[serde(rename = "base64")]
    Base64 { media_type: String, data: String },
    #[serde(rename = "url")]
    Url { url: String },
}

/// Why the model stopped generating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Model finished naturally.
    Stop,
    /// Model wants to call a tool.
    ToolUse,
    /// Hit the max_tokens limit.
    MaxTokens,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn content_text_roundtrip() {
        let content = Content::Text {
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&content).expect("content should serialize to JSON");
        let parsed: Content = serde_json::from_str(&json).expect("JSON should deserialize to content");
        match parsed {
            Content::Text { text } => assert_eq!(text, "hello"),
            other => panic!("Expected Text, got {:?}", other),
        }
    }

    #[test]
    fn content_tool_use_roundtrip() {
        let content = Content::ToolUse {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            input: json!({"command": "ls"}),
        };
        let json = serde_json::to_string(&content).expect("content should serialize to JSON");
        let parsed: Content = serde_json::from_str(&json).expect("JSON should deserialize to content");
        match parsed {
            Content::ToolUse { id, name, input } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "bash");
                assert_eq!(input, json!({"command": "ls"}));
            }
            other => panic!("Expected ToolUse, got {:?}", other),
        }
    }

    #[test]
    fn stop_reason_equality() {
        assert_eq!(StopReason::Stop, StopReason::Stop);
        assert_eq!(StopReason::ToolUse, StopReason::ToolUse);
        assert_ne!(StopReason::Stop, StopReason::ToolUse);
    }

    #[test]
    fn stop_reason_roundtrip() {
        let json = serde_json::to_string(&StopReason::MaxTokens).expect("stop reason should serialize to JSON");
        let parsed: StopReason = serde_json::from_str(&json).expect("JSON should deserialize to stop reason");
        assert_eq!(parsed, StopReason::MaxTokens);
    }
}
