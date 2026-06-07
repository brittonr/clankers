//! Agent-owned hook service contracts.
//!
//! Concrete hook runtimes live at the application edge. The agent only needs a
//! small decision service for lifecycle hooks so reusable turn logic does not
//! depend on a specific hook crate.

use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentHookPoint {
    PrePrompt,
    PostPrompt,
    PreTurn,
    PostTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentHookStatus {
    Pending,
    Success,
    Denied,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentHookSafeError {
    pub message: String,
    pub kind: Option<String>,
}

impl AgentHookSafeError {
    #[must_use]
    pub fn new(message: impl Into<String>, kind: Option<&str>) -> Self {
        Self {
            message: message.into(),
            kind: kind.map(ToOwned::to_owned),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentHookUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentHookData {
    Prompt {
        prompt_id: String,
        text: String,
        system_prompt: Option<String>,
        status: AgentHookStatus,
        error: Option<AgentHookSafeError>,
    },
    Turn {
        prompt_id: String,
        model: String,
        prompt_text: String,
        message_count: u64,
        tool_call_count: u64,
        status: AgentHookStatus,
        error: Option<AgentHookSafeError>,
        usage: Option<AgentHookUsage>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentHookPayload {
    pub event_name: String,
    pub session_id: String,
    pub data: AgentHookData,
}

impl AgentHookPayload {
    #[must_use]
    pub fn prompt_with_metadata(
        event_name: impl Into<String>,
        session_id: impl Into<String>,
        prompt_id: impl Into<String>,
        text: impl Into<String>,
        system_prompt: Option<&str>,
        status: AgentHookStatus,
        error: Option<AgentHookSafeError>,
    ) -> Self {
        Self {
            event_name: event_name.into(),
            session_id: session_id.into(),
            data: AgentHookData::Prompt {
                prompt_id: prompt_id.into(),
                text: text.into(),
                system_prompt: system_prompt.map(ToOwned::to_owned),
                status,
                error,
            },
        }
    }

    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn turn(
        event_name: impl Into<String>,
        session_id: impl Into<String>,
        prompt_id: impl Into<String>,
        model: impl Into<String>,
        prompt_text: impl Into<String>,
        message_count: u64,
        tool_call_count: u64,
        status: AgentHookStatus,
        error: Option<AgentHookSafeError>,
        usage: Option<AgentHookUsage>,
    ) -> Self {
        Self {
            event_name: event_name.into(),
            session_id: session_id.into(),
            data: AgentHookData::Turn {
                prompt_id: prompt_id.into(),
                model: model.into(),
                prompt_text: prompt_text.into(),
                message_count,
                tool_call_count,
                status,
                error,
                usage,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentHookVerdict {
    Continue,
    Modify(Value),
    Deny { reason: String },
}

#[async_trait]
pub trait AgentHookService: Send + Sync {
    async fn fire(&self, point: AgentHookPoint, payload: &AgentHookPayload) -> AgentHookVerdict;

    fn fire_async(&self, point: AgentHookPoint, payload: AgentHookPayload);
}

#[cfg(test)]
fn test_hook_point_from_agent(point: AgentHookPoint) -> clankers_hooks::HookPoint {
    match point {
        AgentHookPoint::PrePrompt => clankers_hooks::HookPoint::PrePrompt,
        AgentHookPoint::PostPrompt => clankers_hooks::HookPoint::PostPrompt,
        AgentHookPoint::PreTurn => clankers_hooks::HookPoint::PreTurn,
        AgentHookPoint::PostTurn => clankers_hooks::HookPoint::PostTurn,
    }
}

#[cfg(test)]
fn test_hook_status_from_agent(status: AgentHookStatus) -> clankers_hooks::HookStatus {
    match status {
        AgentHookStatus::Pending => clankers_hooks::HookStatus::Pending,
        AgentHookStatus::Success => clankers_hooks::HookStatus::Success,
        AgentHookStatus::Denied => clankers_hooks::HookStatus::Denied,
        AgentHookStatus::Error => clankers_hooks::HookStatus::Error,
        AgentHookStatus::Cancelled => clankers_hooks::HookStatus::Cancelled,
    }
}

#[cfg(test)]
fn test_hook_error_from_agent(error: &AgentHookSafeError) -> clankers_hooks::HookSafeError {
    clankers_hooks::HookSafeError::new(&error.message, error.kind.as_deref())
}

#[cfg(test)]
fn test_hook_usage_from_agent(usage: &AgentHookUsage) -> clankers_hooks::HookUsage {
    clankers_hooks::HookUsage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_creation_input_tokens: usage.cache_creation_input_tokens,
        cache_read_input_tokens: usage.cache_read_input_tokens,
    }
}

#[cfg(test)]
fn test_hook_payload_from_agent(payload: &AgentHookPayload) -> clankers_hooks::HookPayload {
    match &payload.data {
        AgentHookData::Prompt {
            prompt_id,
            text,
            system_prompt,
            status,
            error,
        } => clankers_hooks::HookPayload::prompt_with_metadata(
            &payload.event_name,
            &payload.session_id,
            prompt_id,
            text,
            system_prompt.as_deref(),
            test_hook_status_from_agent(*status),
            error.as_ref().map(test_hook_error_from_agent),
        ),
        AgentHookData::Turn {
            prompt_id,
            model,
            prompt_text,
            message_count,
            tool_call_count,
            status,
            error,
            usage,
        } => clankers_hooks::HookPayload::turn(
            &payload.event_name,
            &payload.session_id,
            prompt_id,
            model,
            prompt_text,
            *message_count,
            *tool_call_count,
            test_hook_status_from_agent(*status),
            error.as_ref().map(test_hook_error_from_agent),
            usage.as_ref().map(test_hook_usage_from_agent),
        ),
    }
}

#[cfg(test)]
fn test_hook_verdict_to_agent(verdict: clankers_hooks::HookVerdict) -> AgentHookVerdict {
    match verdict {
        clankers_hooks::HookVerdict::Continue => AgentHookVerdict::Continue,
        clankers_hooks::HookVerdict::Modify(value) => AgentHookVerdict::Modify(value),
        clankers_hooks::HookVerdict::Deny { reason } => AgentHookVerdict::Deny { reason },
    }
}

#[cfg(test)]
#[async_trait]
impl AgentHookService for clankers_hooks::HookPipeline {
    async fn fire(&self, point: AgentHookPoint, payload: &AgentHookPayload) -> AgentHookVerdict {
        let payload = test_hook_payload_from_agent(payload);
        test_hook_verdict_to_agent(
            clankers_hooks::HookPipeline::fire(self, test_hook_point_from_agent(point), &payload).await,
        )
    }

    fn fire_async(&self, point: AgentHookPoint, payload: AgentHookPayload) {
        let payload = test_hook_payload_from_agent(&payload);
        clankers_hooks::HookPipeline::fire_async(self, test_hook_point_from_agent(point), payload);
    }
}
