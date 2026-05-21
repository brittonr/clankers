use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use clankers_provider::CompletionRequest;
use clankers_provider::Provider;
use clankers_provider::message::ToolResultMessage;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use super::CollectedResponse;
use super::execute_tools_parallel;
use super::stream_model_request;
use crate::error::Result;
use crate::events::AgentEvent;
use crate::tool::CapabilityGate;
use crate::tool::Tool;

#[async_trait]
pub(crate) trait AgentModelPort: Send + Sync {
    async fn stream_model_request(
        &self,
        request: CompletionRequest,
        event_tx: &broadcast::Sender<AgentEvent>,
        cancel: &CancellationToken,
    ) -> Result<CollectedResponse>;
}

pub(crate) struct ProviderModelPort<'a> {
    provider: &'a dyn Provider,
}

impl<'a> ProviderModelPort<'a> {
    pub(crate) fn new(provider: &'a dyn Provider) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl AgentModelPort for ProviderModelPort<'_> {
    async fn stream_model_request(
        &self,
        request: CompletionRequest,
        event_tx: &broadcast::Sender<AgentEvent>,
        cancel: &CancellationToken,
    ) -> Result<CollectedResponse> {
        stream_model_request(self.provider, request, event_tx, cancel).await
    }
}

#[async_trait]
pub(crate) trait AgentToolPort: Send + Sync {
    async fn execute_tools(&self, tool_calls: &[(String, String, Value)]) -> Vec<ToolResultMessage>;
}

pub(crate) struct ControllerToolPort<'a> {
    pub(crate) controller_tools: &'a HashMap<String, Arc<dyn Tool>>,
    pub(crate) event_tx: &'a broadcast::Sender<AgentEvent>,
    pub(crate) cancel: CancellationToken,
    pub(crate) hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
    pub(crate) session_id: &'a str,
    pub(crate) db: Option<clankers_db::Db>,
    pub(crate) capability_gate: Option<Arc<dyn CapabilityGate>>,
    pub(crate) user_tool_filter: Option<Vec<String>>,
}

#[async_trait]
impl AgentToolPort for ControllerToolPort<'_> {
    async fn execute_tools(&self, tool_calls: &[(String, String, Value)]) -> Vec<ToolResultMessage> {
        execute_tools_parallel(
            self.controller_tools,
            tool_calls,
            self.event_tx,
            self.cancel.clone(),
            self.hook_pipeline.clone(),
            self.session_id,
            self.db.clone(),
            self.capability_gate.clone(),
            self.user_tool_filter.clone(),
        )
        .await
    }
}
