//! Host-facing runtime event and safe metadata types.

use std::collections::BTreeMap;

pub use clanker_message::ErrorClass;
use clanker_message::SemanticConfirmationRequest;
use clanker_message::SemanticErrorClass;
pub use clanker_message::SemanticStopReason as StopReason;
pub use clanker_message::SemanticToolStatus as ToolStatus;
use clanker_message::SemanticEvent;
use clanker_message::SemanticEventMetadata;
use clanker_message::SemanticStopReason;
use clanker_message::SemanticToolStatus;
use serde::Deserialize;
use serde::Serialize;

use crate::ConfirmationAction;
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
    /// Convert this runtime event into the shared semantic event stream.
    #[must_use]
    pub fn to_semantic_event(&self) -> SemanticEvent {
        match self {
            Self::PromptAccepted { prompt_id, metadata } => SemanticEvent::PromptAccepted {
                prompt_id: prompt_id.as_str().to_string(),
                metadata: semantic_metadata(metadata).with_prompt_id(prompt_id.as_str()),
            },
            Self::ThinkingDelta {
                prompt_id,
                text,
                metadata,
            } => SemanticEvent::ThinkingDelta {
                text: text.clone(),
                metadata: semantic_metadata(metadata).with_prompt_id(prompt_id.as_str()),
            },
            Self::AssistantDelta {
                prompt_id,
                text,
                metadata,
            } => SemanticEvent::AssistantDelta {
                text: text.clone(),
                metadata: semantic_metadata(metadata).with_prompt_id(prompt_id.as_str()),
            },
            Self::ToolStarted {
                prompt_id,
                call_id,
                tool_name,
                metadata,
            } => SemanticEvent::ToolStarted {
                call_id: call_id.clone(),
                tool_name: tool_name.clone(),
                metadata: semantic_metadata(metadata).with_prompt_id(prompt_id.as_str()),
            },
            Self::ToolFinished {
                prompt_id,
                call_id,
                status,
                metadata,
            } => SemanticEvent::ToolFinished {
                call_id: call_id.clone(),
                status: semantic_tool_status(*status),
                text: String::new(),
                images: Vec::new(),
                metadata: semantic_metadata(metadata).with_prompt_id(prompt_id.as_str()),
            },
            Self::ConfirmationRequested {
                prompt_id,
                request,
                metadata,
            } => SemanticEvent::ConfirmationRequested {
                request: semantic_confirmation_request(request),
                metadata: semantic_metadata(metadata).with_prompt_id(prompt_id.as_str()),
            },
            Self::CostUpdated {
                prompt_id,
                input_tokens,
                output_tokens,
                metadata,
            } => SemanticEvent::UsageUpdated {
                input_tokens: *input_tokens,
                output_tokens: *output_tokens,
                cache_read_tokens: 0,
                metadata: semantic_metadata(metadata).with_prompt_id(prompt_id.as_str()),
            },
            Self::Completed {
                prompt_id,
                stop_reason,
                metadata,
            } => SemanticEvent::Completed {
                stop_reason: semantic_stop_reason(*stop_reason),
                metadata: semantic_metadata(metadata).with_prompt_id(prompt_id.as_str()),
            },
            Self::Error {
                prompt_id,
                message,
                error_class,
                metadata,
            } => {
                let metadata = match prompt_id {
                    Some(prompt_id) => semantic_metadata(metadata).with_prompt_id(prompt_id.as_str()),
                    None => semantic_metadata(metadata),
                };
                SemanticEvent::Error {
                    message: sanitize_metadata_value(message.clone()),
                    error_class: semantic_error_class(*error_class),
                    metadata,
                }
            }
            Self::Shutdown { metadata } => SemanticEvent::Shutdown {
                metadata: semantic_metadata(metadata),
            },
        }
    }

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

fn semantic_metadata(metadata: &EventMetadata) -> SemanticEventMetadata {
    let mut semantic = SemanticEventMetadata::empty();
    if let Some(session_id) = &metadata.session_id {
        semantic = semantic.with_session_id(session_id.as_str());
    }
    for (key, value) in &metadata.fields {
        semantic = semantic.with(key.clone(), value.clone());
    }
    semantic
}

fn semantic_confirmation_request(request: &ConfirmationRequest) -> SemanticConfirmationRequest {
    SemanticConfirmationRequest {
        request_id: request.id.clone(),
        action: confirmation_action_name(&request.action),
        summary: sanitize_metadata_value(request.summary.clone()),
        working_dir: request.metadata.fields.get("working_dir").cloned().map(sanitize_metadata_value),
    }
}

fn confirmation_action_name(action: &ConfirmationAction) -> String {
    match action {
        ConfirmationAction::RunCommand => "run_command".to_string(),
        ConfirmationAction::MutateWorkspace => "mutate_workspace".to_string(),
        ConfirmationAction::UseNetwork => "use_network".to_string(),
        ConfirmationAction::Custom(name) => sanitize_metadata_value(name.clone()),
    }
}

fn semantic_tool_status(status: ToolStatus) -> SemanticToolStatus {
    match status {
        ToolStatus::Succeeded => SemanticToolStatus::Succeeded,
        ToolStatus::Failed => SemanticToolStatus::Failed,
        ToolStatus::Denied => SemanticToolStatus::Denied,
    }
}

fn semantic_stop_reason(stop_reason: StopReason) -> SemanticStopReason {
    match stop_reason {
        StopReason::Complete => SemanticStopReason::Complete,
        StopReason::Interrupted => SemanticStopReason::Interrupted,
        StopReason::Cancelled => SemanticStopReason::Cancelled,
    }
}

fn semantic_error_class(error_class: ErrorClass) -> SemanticErrorClass {
    match error_class {
        ErrorClass::InvalidInput => SemanticErrorClass::InvalidInput,
        ErrorClass::Session => SemanticErrorClass::Session,
        ErrorClass::Policy => SemanticErrorClass::Policy,
        ErrorClass::Tooling => SemanticErrorClass::Tooling,
        ErrorClass::Storage => SemanticErrorClass::Storage,
        ErrorClass::Confirmation => SemanticErrorClass::Confirmation,
        ErrorClass::Extension => SemanticErrorClass::Extension,
        ErrorClass::Boundary => SemanticErrorClass::Boundary,
        ErrorClass::Model => SemanticErrorClass::Model,
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
