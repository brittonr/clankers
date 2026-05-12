//! Host-facing runtime event and safe metadata types.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

use crate::ConfirmationRequest;
use crate::PromptId;
use crate::SessionId;

/// Semantic session events for host applications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    PromptAccepted {
        prompt_id: PromptId,
        metadata: EventMetadata,
    },
    ThinkingDelta {
        prompt_id: PromptId,
        text: String,
        metadata: EventMetadata,
    },
    AssistantDelta {
        prompt_id: PromptId,
        text: String,
        metadata: EventMetadata,
    },
    ToolStarted {
        prompt_id: PromptId,
        call_id: String,
        tool_name: String,
        metadata: EventMetadata,
    },
    ToolFinished {
        prompt_id: PromptId,
        call_id: String,
        status: ToolStatus,
        metadata: EventMetadata,
    },
    ConfirmationRequested {
        prompt_id: PromptId,
        request: ConfirmationRequest,
        metadata: EventMetadata,
    },
    CostUpdated {
        prompt_id: PromptId,
        input_tokens: u64,
        output_tokens: u64,
        metadata: EventMetadata,
    },
    Completed {
        prompt_id: PromptId,
        stop_reason: StopReason,
        metadata: EventMetadata,
    },
    Error {
        prompt_id: Option<PromptId>,
        message: String,
        error_class: ErrorClass,
        metadata: EventMetadata,
    },
    Shutdown {
        metadata: EventMetadata,
    },
}

impl SessionEvent {
    pub(crate) fn with_session_metadata(self, session_id: SessionId, prompt_id: PromptId) -> Self {
        match self {
            Self::AssistantDelta { text, metadata, .. } => Self::AssistantDelta {
                prompt_id,
                text,
                metadata: metadata.with_session(session_id),
            },
            Self::ThinkingDelta { text, metadata, .. } => Self::ThinkingDelta {
                prompt_id,
                text,
                metadata: metadata.with_session(session_id),
            },
            Self::ToolStarted {
                call_id,
                tool_name,
                metadata,
                ..
            } => Self::ToolStarted {
                prompt_id,
                call_id,
                tool_name,
                metadata: metadata.with_session(session_id),
            },
            Self::ToolFinished {
                call_id,
                status,
                metadata,
                ..
            } => Self::ToolFinished {
                prompt_id,
                call_id,
                status,
                metadata: metadata.with_session(session_id),
            },
            Self::CostUpdated {
                input_tokens,
                output_tokens,
                metadata,
                ..
            } => Self::CostUpdated {
                prompt_id,
                input_tokens,
                output_tokens,
                metadata: metadata.with_session(session_id),
            },
            event => event,
        }
    }
}

/// Safe replay/routing metadata. Values are constrained to strings selected by runtime code.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventMetadata {
    pub session_id: Option<SessionId>,
    pub fields: BTreeMap<String, String>,
}

impl EventMetadata {
    #[must_use]
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id: Some(session_id),
            fields: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key: String = key.into().chars().take(160).collect();
        let value = value.into();
        let value = if contains_secret_marker(&key) {
            "[REDACTED]".to_string()
        } else {
            sanitize_metadata_value(value)
        };
        self.fields.insert(key, value);
        self
    }

    #[must_use]
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    #[must_use]
    pub fn contains_secret_markers(&self) -> bool {
        self.fields.values().any(|value| contains_secret_marker(value))
    }
}

pub(crate) fn sanitize_metadata_value(value: String) -> String {
    if contains_secret_marker(&value) {
        "[REDACTED]".to_string()
    } else {
        value.chars().take(160).collect()
    }
}

pub(crate) fn contains_secret_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "api_key",
        "authorization",
        "bearer",
        "cookie",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Complete,
    Interrupted,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Succeeded,
    Failed,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    InvalidInput,
    Session,
    Policy,
    Tooling,
    Storage,
    Confirmation,
    Extension,
    Boundary,
    Model,
}
