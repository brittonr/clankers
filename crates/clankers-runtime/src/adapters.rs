//! Host-facing runtime adapter traits for engine-host execution.
//!
//! These traits are object-safe shims used by `RuntimeBuilder` so embedded hosts can
//! replace model-adjacent effects without depending on Clankers desktop shells.

use clanker_message::Content;
pub use clanker_message::RuntimeRetryRequest;
pub use clanker_message::RuntimeUsageObservation;
pub use clanker_message::RuntimeUsageObservationKind;
use clankers_engine::EngineEvent;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::PromptId;
use crate::RuntimeError;
use crate::SessionId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeToolRequest {
    pub session_id: SessionId,
    pub prompt_id: PromptId,
    pub call_id: String,
    pub tool_name: String,
    #[serde(default)]
    pub input: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeToolStatus {
    Succeeded,
    Failed,
    Missing,
    Denied,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeToolResponse {
    pub status: RuntimeToolStatus,
    #[serde(default)]
    pub content: Vec<Content>,
    #[serde(default)]
    pub details: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl RuntimeToolResponse {
    #[must_use]
    pub fn succeeded(content: Vec<Content>, details: Value) -> Self {
        Self {
            status: RuntimeToolStatus::Succeeded,
            content,
            details,
            message: None,
        }
    }

    #[must_use]
    pub fn failed(message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            status: RuntimeToolStatus::Failed,
            content: vec![Content::Text { text: message.clone() }],
            details: Value::Null,
            message: Some(message),
        }
    }
}

pub trait RuntimeToolAdapter: Send + Sync {
    fn execute_tool(&self, request: RuntimeToolRequest) -> Result<RuntimeToolResponse, RuntimeError>;
}

pub struct UnavailableRuntimeToolAdapter;

impl RuntimeToolAdapter for UnavailableRuntimeToolAdapter {
    fn execute_tool(&self, request: RuntimeToolRequest) -> Result<RuntimeToolResponse, RuntimeError> {
        let _ = request;
        Err(RuntimeError::ExtensionUnavailable("runtime tool adapter unavailable".to_string()))
    }
}

pub trait RuntimeRetryAdapter: Send + Sync {
    fn sleep_for_retry(&self, request: RuntimeRetryRequest) -> Result<(), RuntimeError>;
}

pub struct NoopRuntimeRetryAdapter;

impl RuntimeRetryAdapter for NoopRuntimeRetryAdapter {
    fn sleep_for_retry(&self, request: RuntimeRetryRequest) -> Result<(), RuntimeError> {
        let _ = request;
        Ok(())
    }
}

pub trait RuntimeEventObserver: Send + Sync {
    fn observe_engine_event(&self, event: &EngineEvent) -> Result<(), RuntimeError>;
}

pub struct NoopRuntimeEventObserver;

impl RuntimeEventObserver for NoopRuntimeEventObserver {
    fn observe_engine_event(&self, event: &EngineEvent) -> Result<(), RuntimeError> {
        let _ = event;
        Ok(())
    }
}

pub trait RuntimeCancellationAdapter: Send + Sync {
    fn is_cancelled(&self) -> bool;

    fn cancellation_reason(&self) -> String {
        "runtime cancelled".to_string()
    }
}

pub struct NoopRuntimeCancellationAdapter;

impl RuntimeCancellationAdapter for NoopRuntimeCancellationAdapter {
    fn is_cancelled(&self) -> bool {
        false
    }
}

pub trait RuntimeUsageAdapter: Send + Sync {
    fn observe_usage(&self, observation: RuntimeUsageObservation) -> Result<(), RuntimeError>;
}

pub struct NoopRuntimeUsageAdapter;

impl RuntimeUsageAdapter for NoopRuntimeUsageAdapter {
    fn observe_usage(&self, observation: RuntimeUsageObservation) -> Result<(), RuntimeError> {
        let _ = observation;
        Ok(())
    }
}
