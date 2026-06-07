//! Agent-owned model streaming service contracts.

use std::collections::HashMap;

use async_trait::async_trait;
use clanker_message::ThinkingConfig;
use clanker_message::ToolDefinition;
use clanker_message::streaming::StreamEvent;
use clanker_message::transcript::AgentMessage;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCompletionRequest {
    pub model: String,
    pub messages: Vec<AgentMessage>,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
    pub tools: Vec<ToolDefinition>,
    pub thinking: Option<ThinkingConfig>,
    #[serde(default)]
    pub no_cache: bool,
    #[serde(default)]
    pub cache_ttl: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_params: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct AgentModelError {
    pub message: String,
    pub status: Option<u16>,
    pub retryable: bool,
    pub should_compress: bool,
}

impl AgentModelError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: None,
            retryable: false,
            should_compress: false,
        }
    }

    #[must_use]
    pub fn with_status(mut self, status: Option<u16>) -> Self {
        self.status = status;
        self
    }

    #[must_use]
    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    #[must_use]
    pub fn should_compress(mut self, should_compress: bool) -> Self {
        self.should_compress = should_compress;
        self
    }
}

impl std::fmt::Display for AgentModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AgentModelError {}

pub type AgentModelResult<T> = std::result::Result<T, AgentModelError>;

#[cfg(test)]
fn provider_request_from_agent(request: AgentCompletionRequest) -> clankers_provider::CompletionRequest {
    clankers_provider::CompletionRequest {
        model: request.model,
        messages: request.messages,
        system_prompt: request.system_prompt,
        max_tokens: request.max_tokens,
        temperature: request.temperature,
        tools: request.tools,
        thinking: request.thinking,
        no_cache: request.no_cache,
        cache_ttl: request.cache_ttl,
        extra_params: request.extra_params,
    }
}

#[cfg(test)]
fn agent_error_from_provider(error: clankers_provider::error::ProviderError) -> AgentModelError {
    let retryable = error.is_retryable();
    let should_compress = error.should_compress();
    let status = error.status;
    AgentModelError::new(error.message).with_status(status).retryable(retryable).should_compress(should_compress)
}

#[async_trait]
pub trait AgentModelService: Send + Sync {
    async fn complete(&self, request: AgentCompletionRequest, tx: mpsc::Sender<StreamEvent>) -> AgentModelResult<()>;

    fn name(&self) -> &str {
        "model"
    }

    fn max_input_tokens(&self, _model: &str) -> Option<usize> {
        None
    }

    async fn reload_credentials(&self) {}
}

#[cfg(test)]
#[async_trait]
impl<T> AgentModelService for T
where
    T: clankers_provider::Provider + ?Sized,
{
    async fn complete(&self, request: AgentCompletionRequest, tx: mpsc::Sender<StreamEvent>) -> AgentModelResult<()> {
        clankers_provider::Provider::complete(self, provider_request_from_agent(request), tx)
            .await
            .map_err(agent_error_from_provider)
    }

    fn name(&self) -> &str {
        clankers_provider::Provider::name(self)
    }

    fn max_input_tokens(&self, model: &str) -> Option<usize> {
        clankers_provider::Provider::models(self)
            .iter()
            .find(|candidate| candidate.id == model)
            .map(|candidate| candidate.max_input_tokens)
    }

    async fn reload_credentials(&self) {
        clankers_provider::Provider::reload_credentials(self).await;
    }
}

pub fn thinking_level_to_config(level: clanker_message::ThinkingLevel) -> Option<ThinkingConfig> {
    if level.is_enabled() {
        Some(ThinkingConfig {
            enabled: true,
            budget_tokens: level.budget_tokens().map(|tokens| tokens as usize),
        })
    } else {
        None
    }
}
