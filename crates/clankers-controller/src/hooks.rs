//! Controller-owned hook service port.
//!
//! Concrete hook runtimes live at the root/daemon edge. The controller only
//! emits neutral hook intents so reusable controller orchestration does not
//! depend on the desktop hook pipeline crate.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControllerHookPoint {
    PrePrompt,
    PostPrompt,
    SessionStart,
    SessionEnd,
    PreTurn,
    TurnStart,
    TurnEnd,
    PostTurn,
    ModelChange,
}

#[derive(Debug, Clone, Default)]
pub enum ControllerHookVerdict {
    #[default]
    Continue,
    Modify(Value),
    Deny { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControllerHookStatus {
    #[default]
    Pending,
    Success,
    Denied,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerHookSafeError {
    pub message: String,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ControllerHookUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct ControllerHookPayload {
    pub event_name: String,
    pub session_id: String,
    pub data: ControllerHookData,
}

impl ControllerHookPayload {
    #[must_use]
    pub fn session(event_name: &str, session_id: &str) -> Self {
        Self {
            event_name: event_name.to_string(),
            session_id: session_id.to_string(),
            data: ControllerHookData::Session {
                session_id: session_id.to_string(),
            },
        }
    }

    #[must_use]
    pub fn empty(event_name: &str, session_id: &str) -> Self {
        Self {
            event_name: event_name.to_string(),
            session_id: session_id.to_string(),
            data: ControllerHookData::Empty,
        }
    }

    #[must_use]
    pub fn model_change(event_name: &str, session_id: &str, from: &str, to: &str, reason: &str) -> Self {
        Self {
            event_name: event_name.to_string(),
            session_id: session_id.to_string(),
            data: ControllerHookData::ModelChange {
                from: from.to_string(),
                to: to.to_string(),
                reason: reason.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum ControllerHookData {
    Prompt {
        prompt_id: String,
        text: String,
        system_prompt: Option<String>,
        status: ControllerHookStatus,
        error: Option<ControllerHookSafeError>,
    },
    Turn {
        prompt_id: String,
        model: String,
        prompt_text: String,
        message_count: u64,
        tool_call_count: u64,
        status: ControllerHookStatus,
        error: Option<ControllerHookSafeError>,
        usage: Option<ControllerHookUsage>,
    },
    Session {
        session_id: String,
    },
    ModelChange {
        from: String,
        to: String,
        reason: String,
    },
    Empty,
}

#[async_trait::async_trait]
pub trait ControllerHookService: Send + Sync {
    async fn fire(&self, point: ControllerHookPoint, payload: &ControllerHookPayload) -> ControllerHookVerdict;

    fn fire_async(&self, point: ControllerHookPoint, payload: ControllerHookPayload);
}
