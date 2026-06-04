//! Reusable semantic session event stream contracts.
//!
//! These DTOs sit below controller, daemon, TUI, Matrix, provider, and shell
//! adapters. They describe session behavior in serializable terms without
//! carrying transport frames, display widgets, provider-native payloads, or
//! hidden prompt context.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

/// Stable semantic events emitted by reusable session runtimes and projected at shell edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SemanticEvent {
    PromptAccepted {
        prompt_id: String,
        metadata: SemanticEventMetadata,
    },
    AgentStart {
        metadata: SemanticEventMetadata,
    },
    AgentEnd {
        metadata: SemanticEventMetadata,
    },
    ContentBlockStart {
        is_thinking: bool,
        metadata: SemanticEventMetadata,
    },
    ContentBlockStop {
        metadata: SemanticEventMetadata,
    },
    AssistantDelta {
        text: String,
        metadata: SemanticEventMetadata,
    },
    ThinkingDelta {
        text: String,
        metadata: SemanticEventMetadata,
    },
    ToolCall {
        tool_name: String,
        call_id: String,
        input: Value,
        metadata: SemanticEventMetadata,
    },
    ToolStarted {
        call_id: String,
        tool_name: String,
        metadata: SemanticEventMetadata,
    },
    ToolOutput {
        call_id: String,
        text: String,
        images: Vec<SemanticImage>,
        metadata: SemanticEventMetadata,
    },
    ToolProgressUpdate {
        call_id: String,
        message: Option<String>,
        metadata: SemanticEventMetadata,
    },
    ToolChunk {
        call_id: String,
        content: String,
        content_type: String,
        metadata: SemanticEventMetadata,
    },
    ToolFinished {
        call_id: String,
        status: SemanticToolStatus,
        text: String,
        images: Vec<SemanticImage>,
        metadata: SemanticEventMetadata,
    },
    ConfirmationRequested {
        request: SemanticConfirmationRequest,
        metadata: SemanticEventMetadata,
    },
    UsageUpdated {
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        metadata: SemanticEventMetadata,
    },
    Error {
        message: String,
        error_class: SemanticErrorClass,
        metadata: SemanticEventMetadata,
    },
    Completed {
        stop_reason: SemanticStopReason,
        metadata: SemanticEventMetadata,
    },
    Shutdown {
        metadata: SemanticEventMetadata,
    },
    UserInput {
        text: String,
        agent_msg_count: usize,
        timestamp_rfc3339: String,
        metadata: SemanticEventMetadata,
    },
    SessionCompaction {
        compacted_count: usize,
        tokens_saved: usize,
        metadata: SemanticEventMetadata,
    },
}

impl SemanticEvent {
    /// Return the stable snake_case event kind used by receipts and projections.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::PromptAccepted { .. } => "prompt_accepted",
            Self::AgentStart { .. } => "agent_start",
            Self::AgentEnd { .. } => "agent_end",
            Self::ContentBlockStart { .. } => "content_block_start",
            Self::ContentBlockStop { .. } => "content_block_stop",
            Self::AssistantDelta { .. } => "assistant_delta",
            Self::ThinkingDelta { .. } => "thinking_delta",
            Self::ToolCall { .. } => "tool_call",
            Self::ToolStarted { .. } => "tool_started",
            Self::ToolOutput { .. } => "tool_output",
            Self::ToolProgressUpdate { .. } => "tool_progress_update",
            Self::ToolChunk { .. } => "tool_chunk",
            Self::ToolFinished { .. } => "tool_finished",
            Self::ConfirmationRequested { .. } => "confirmation_requested",
            Self::UsageUpdated { .. } => "usage_updated",
            Self::Error { .. } => "error",
            Self::Completed { .. } => "completed",
            Self::Shutdown { .. } => "shutdown",
            Self::UserInput { .. } => "user_input",
            Self::SessionCompaction { .. } => "session_compaction",
        }
    }

    /// Return the metadata carried by this event.
    #[must_use]
    pub fn metadata(&self) -> &SemanticEventMetadata {
        match self {
            Self::PromptAccepted { metadata, .. }
            | Self::AgentStart { metadata }
            | Self::AgentEnd { metadata }
            | Self::ContentBlockStart { metadata, .. }
            | Self::ContentBlockStop { metadata }
            | Self::AssistantDelta { metadata, .. }
            | Self::ThinkingDelta { metadata, .. }
            | Self::ToolCall { metadata, .. }
            | Self::ToolStarted { metadata, .. }
            | Self::ToolOutput { metadata, .. }
            | Self::ToolProgressUpdate { metadata, .. }
            | Self::ToolChunk { metadata, .. }
            | Self::ToolFinished { metadata, .. }
            | Self::ConfirmationRequested { metadata, .. }
            | Self::UsageUpdated { metadata, .. }
            | Self::Error { metadata, .. }
            | Self::Completed { metadata, .. }
            | Self::Shutdown { metadata }
            | Self::UserInput { metadata, .. }
            | Self::SessionCompaction { metadata, .. } => metadata,
        }
    }
}

/// Safe event metadata selected by runtime/controller code.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticEventMetadata {
    pub session_id: Option<String>,
    pub prompt_id: Option<String>,
    pub fields: BTreeMap<String, String>,
}

impl SemanticEventMetadata {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(sanitize_semantic_metadata_value(session_id.into()));
        self
    }

    #[must_use]
    pub fn with_prompt_id(mut self, prompt_id: impl Into<String>) -> Self {
        self.prompt_id = Some(sanitize_semantic_metadata_value(prompt_id.into()));
        self
    }

    #[must_use]
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key: String = key.into().chars().take(160).collect();
        let value = value.into();
        let value = if contains_secret_marker(&key) {
            "[REDACTED]".to_string()
        } else {
            sanitize_semantic_metadata_value(value)
        };
        self.fields.insert(key, value);
        self
    }

    #[must_use]
    pub fn contains_secret_markers(&self) -> bool {
        self.session_id
            .iter()
            .chain(self.prompt_id.iter())
            .chain(self.fields.values())
            .any(|value| contains_secret_marker(value))
    }
}

/// Display-neutral image payload for semantic tool output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticImage {
    pub data: String,
    pub media_type: String,
}

/// Confirmation request information safe for host projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticConfirmationRequest {
    pub request_id: String,
    pub action: String,
    pub summary: String,
    pub working_dir: Option<String>,
}

/// Terminal status of a semantic tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticToolStatus {
    Succeeded,
    Failed,
    Denied,
}

/// High-level stop reason for semantic turn completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticStopReason {
    Complete,
    Interrupted,
    Cancelled,
}

/// Safe error class for semantic event projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticErrorClass {
    InvalidInput,
    Session,
    Policy,
    Tooling,
    Storage,
    Confirmation,
    Extension,
    Boundary,
    Model,
    Unknown,
}

fn sanitize_semantic_metadata_value(value: String) -> String {
    if contains_secret_marker(&value) {
        "[REDACTED]".to_string()
    } else {
        value.chars().take(160).collect()
    }
}

fn contains_secret_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "api_key",
        "authorization",
        "bearer",
        "cookie",
        "hidden prompt",
        "provider payload",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_event_ordering_fixture_covers_core_behavior() {
        let metadata = SemanticEventMetadata::empty()
            .with_session_id("session-1")
            .with_prompt_id("prompt-1")
            .with("source", "fixture");
        let events = vec![
            SemanticEvent::PromptAccepted {
                prompt_id: "prompt-1".to_string(),
                metadata: metadata.clone(),
            },
            SemanticEvent::AssistantDelta {
                text: "hello".to_string(),
                metadata: metadata.clone(),
            },
            SemanticEvent::ThinkingDelta {
                text: "thinking".to_string(),
                metadata: metadata.clone(),
            },
            SemanticEvent::ToolStarted {
                call_id: "call-1".to_string(),
                tool_name: "bash".to_string(),
                metadata: metadata.clone(),
            },
            SemanticEvent::ToolFinished {
                call_id: "call-1".to_string(),
                status: SemanticToolStatus::Succeeded,
                text: "done".to_string(),
                images: Vec::new(),
                metadata: metadata.clone(),
            },
            SemanticEvent::ConfirmationRequested {
                request: SemanticConfirmationRequest {
                    request_id: "confirm-1".to_string(),
                    action: "bash".to_string(),
                    summary: "run command".to_string(),
                    working_dir: Some("/repo".to_string()),
                },
                metadata: metadata.clone(),
            },
            SemanticEvent::UsageUpdated {
                input_tokens: 10,
                output_tokens: 20,
                cache_read_tokens: 3,
                metadata: metadata.clone(),
            },
            SemanticEvent::Error {
                message: "model unavailable".to_string(),
                error_class: SemanticErrorClass::Model,
                metadata: metadata.clone(),
            },
            SemanticEvent::Completed {
                stop_reason: SemanticStopReason::Complete,
                metadata,
            },
        ];

        let kinds: Vec<&str> = events.iter().map(SemanticEvent::kind).collect();
        assert_eq!(kinds, vec![
            "prompt_accepted",
            "assistant_delta",
            "thinking_delta",
            "tool_started",
            "tool_finished",
            "confirmation_requested",
            "usage_updated",
            "error",
            "completed"
        ]);
    }

    #[test]
    fn semantic_event_metadata_redacts_secret_markers() {
        let metadata = SemanticEventMetadata::empty()
            .with_session_id("session-secret-token")
            .with_prompt_id("prompt-hidden prompt")
            .with("authorization", "Bearer SECRET_TOKEN")
            .with("safe", "visible");
        let event = SemanticEvent::Error {
            message: "safe error".to_string(),
            error_class: SemanticErrorClass::Policy,
            metadata,
        };
        let value = serde_json::to_string(&event).expect("semantic event serializes");
        assert!(!value.contains("SECRET_TOKEN"));
        assert!(!value.contains("hidden prompt"));
        assert!(!value.contains("session-secret-token"));
        assert!(value.contains("[REDACTED]"));
        assert!(!event.metadata().contains_secret_markers());
    }
}
